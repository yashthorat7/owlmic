// JNI side of com.owlmic.media.audio.Oboe: the mic's blocking input stream and the speaker's
// real-time callback output stream backed by a lock-free SPSC PCM ring buffer.
#include <jni.h>
#include <oboe/Oboe.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <memory>
#include <mutex>
#include <vector>

namespace {

constexpr int kSampleRate = 48000;
constexpr int kMaxFrames = 4800;
constexpr int kOutputRingFrames = 16384; // power of two (~341 ms at 48 kHz)
constexpr int kOutputRingMask = kOutputRingFrames - 1;
constexpr int kDefaultFrameSamples = 480; // 10 ms at 48 kHz
constexpr int kUnderrunFadeFrames = 64;

struct OutputRing : public oboe::AudioStreamDataCallback, public oboe::AudioStreamErrorCallback {
    std::vector<int16_t> ring;
    int channels = 2;
    int maxQueuedFrames = kDefaultFrameSamples * 2;
    std::atomic<uint64_t> readIdx{0};
    std::atomic<uint64_t> writeIdx{0};
    std::atomic<bool> disconnected{false};
    std::atomic<bool> closing{false};
    std::atomic<bool> waiting{false};
    std::mutex waitMu;
    std::condition_variable waitCv;
    int16_t lastSample[2] = {0, 0};

    oboe::DataCallbackResult onAudioReady(oboe::AudioStream *, void *audioData, int32_t numFrames) override {
        auto *out = static_cast<int16_t *>(audioData);
        const int ch = channels;
        const uint64_t r = readIdx.load(std::memory_order_relaxed);
        const uint64_t w = writeIdx.load(std::memory_order_acquire);
        const int avail = static_cast<int>(w - r);
        const int take = std::min(avail, static_cast<int>(numFrames));

        for (int i = 0; i < take; ++i) {
            const size_t idx = static_cast<size_t>((r + i) & kOutputRingMask) * ch;
            for (int c = 0; c < ch; ++c) {
                out[i * ch + c] = ring[idx + c];
            }
        }
        if (take > 0) {
            for (int c = 0; c < std::min(ch, 2); ++c) {
                lastSample[c] = out[(take - 1) * ch + c];
            }
            readIdx.store(r + take, std::memory_order_release);
        }
        if (take < numFrames) {
            const int rem = numFrames - take;
            const int fade = std::min(rem, kUnderrunFadeFrames);
            for (int i = 0; i < fade; ++i) {
                const float g = static_cast<float>(fade - 1 - i) / static_cast<float>(fade);
                for (int c = 0; c < ch; ++c) {
                    const int16_t s = c < 2 ? lastSample[c] : lastSample[0];
                    out[(take + i) * ch + c] = static_cast<int16_t>(s * g);
                }
            }
            if (rem > fade) {
                std::fill_n(out + (take + fade) * ch, (rem - fade) * ch, int16_t{0});
            }
            lastSample[0] = 0;
            lastSample[1] = 0;
        }
        if (waiting.load(std::memory_order_relaxed)) {
            std::unique_lock<std::mutex> lk(waitMu, std::try_to_lock);
            waitCv.notify_one();
        }
        return oboe::DataCallbackResult::Continue;
    }

    void onErrorAfterClose(oboe::AudioStream *, oboe::Result) override {
        disconnected.store(true, std::memory_order_release);
        {
            std::unique_lock<std::mutex> lk(waitMu, std::try_to_lock);
        }
        waitCv.notify_all();
    }
};

struct Holder {
    std::shared_ptr<oboe::AudioStream> stream;
    std::vector<int16_t> buffer;
    std::shared_ptr<OutputRing> output;
};

Holder *holder(jlong handle) { return reinterpret_cast<Holder *>(handle); }

// The low latency path skips the platform's voice processing (its echo canceller among it) on many phones, so a stream
// that needs it asks for the normal path.
oboe::PerformanceMode mode(jboolean lowLatency) {
    return lowLatency ? oboe::PerformanceMode::LowLatency : oboe::PerformanceMode::None;
}

}  // namespace

extern "C" {

// preset: an AAudio input preset (6 voice recognition, 7 voice communication). The stream gets its own audio session,
// so the Java side can attach or switch off the platform's effects on it.
JNIEXPORT jlong JNICALL Java_com_owlmic_media_audio_Oboe_openInput(JNIEnv *, jclass, jint preset, jboolean lowLatency) {
    oboe::AudioStreamBuilder builder;
    builder.setDirection(oboe::Direction::Input)
        ->setSharingMode(oboe::SharingMode::Shared)
        ->setPerformanceMode(mode(lowLatency))
        ->setFormat(oboe::AudioFormat::I16)
        ->setChannelCount(1)
        ->setSampleRate(kSampleRate)
        ->setSampleRateConversionQuality(oboe::SampleRateConversionQuality::Medium)
        ->setInputPreset(static_cast<oboe::InputPreset>(preset))
        ->setSessionId(oboe::SessionId::Allocate);
    std::shared_ptr<oboe::AudioStream> stream;
    if (builder.openStream(stream) != oboe::Result::OK) return 0;
    if (stream->requestStart() != oboe::Result::OK) {
        stream->close();
        return 0;
    }
    auto *h = new Holder{stream, std::vector<int16_t>(kMaxFrames), nullptr};
    return reinterpret_cast<jlong>(h);
}

// usage: 1 media, 2 voice communication. contentType: 1 speech, 2 music.
JNIEXPORT jlong JNICALL Java_com_owlmic_media_audio_Oboe_openOutput(JNIEnv *, jclass, jint channels, jint usage, jint contentType, jboolean lowLatency) {
    auto out = std::make_shared<OutputRing>();
    out->channels = channels;
    out->ring.assign(static_cast<size_t>(kOutputRingFrames) * channels, 0);

    oboe::AudioStreamBuilder builder;
    builder.setDirection(oboe::Direction::Output)
        ->setSharingMode(oboe::SharingMode::Shared)
        ->setPerformanceMode(mode(lowLatency))
        ->setFormat(oboe::AudioFormat::I16)
        ->setChannelCount(channels)
        ->setSampleRate(kSampleRate)
        ->setSampleRateConversionQuality(oboe::SampleRateConversionQuality::Medium)
        ->setUsage(static_cast<oboe::Usage>(usage))
        ->setContentType(static_cast<oboe::ContentType>(contentType))
        ->setDataCallback(out)
        ->setErrorCallback(out);

    std::shared_ptr<oboe::AudioStream> stream;
    if (builder.openStream(stream) != oboe::Result::OK) return 0;

    const int32_t burst = std::max(1, stream->getFramesPerBurst());
    stream->setBufferSizeInFrames(burst * 2);
    out->maxQueuedFrames = std::min(kOutputRingFrames / 2, std::max(kDefaultFrameSamples * 2, burst * 2));

    if (stream->requestStart() != oboe::Result::OK) {
        stream->close();
        return 0;
    }
    auto *h = new Holder{stream, std::vector<int16_t>(kMaxFrames * channels), out};
    return reinterpret_cast<jlong>(h);
}

JNIEXPORT jint JNICALL Java_com_owlmic_media_audio_Oboe_sessionId(JNIEnv *, jclass, jlong handle) {
    return holder(handle)->stream->getSessionId();
}

// Returns frames read (0 on timeout), or a negative oboe::Result (for example -899 when the device went away).
JNIEXPORT jint JNICALL Java_com_owlmic_media_audio_Oboe_read(JNIEnv *env, jclass, jlong handle, jshortArray out, jint frames, jint timeoutMs) {
    Holder *h = holder(handle);
    if (frames > kMaxFrames) frames = kMaxFrames;
    auto result = h->stream->read(h->buffer.data(), frames, static_cast<int64_t>(timeoutMs) * 1000000);
    if (!result) return static_cast<jint>(result.error());
    int got = result.value();
    if (got > 0) {
        int channels = h->stream->getChannelCount();
        if (channels <= 1) {
            env->SetShortArrayRegion(out, 0, got, h->buffer.data());
        } else {
            jsize outLen = env->GetArrayLength(out);
            int toCopy = got < outLen ? got : outLen;
            std::vector<int16_t> mono(toCopy);
            for (int i = 0; i < toCopy; ++i) {
                int32_t sum = 0;
                for (int c = 0; c < channels; ++c) {
                    sum += h->buffer[i * channels + c];
                }
                mono[i] = static_cast<int16_t>(sum / channels);
            }
            env->SetShortArrayRegion(out, 0, toCopy, mono.data());
            return toCopy;
        }
    }
    return got;
}

// Writes frames into the lock-free output ring, pacing the caller once target headroom is buffered.
// Returns frames written, or a negative oboe::Result (e.g. -899 on disconnect).
JNIEXPORT jint JNICALL Java_com_owlmic_media_audio_Oboe_write(JNIEnv *env, jclass, jlong handle, jshortArray in, jint frames, jint timeoutMs) {
    Holder *h = holder(handle);
    OutputRing *out = h->output.get();
    if (out == nullptr || frames <= 0) return 0;
    if (frames > kMaxFrames) frames = kMaxFrames;
    const int ch = out->channels;
    const int targetCap = std::max(out->maxQueuedFrames, frames * 2);

    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeoutMs);
    while (true) {
        if (out->disconnected.load(std::memory_order_acquire)) {
            return static_cast<jint>(oboe::Result::ErrorDisconnected);
        }
        if (out->closing.load(std::memory_order_acquire)) return 0;
        const uint64_t r = out->readIdx.load(std::memory_order_acquire);
        const uint64_t w = out->writeIdx.load(std::memory_order_relaxed);
        const int queued = static_cast<int>(w - r);
        if (queued + frames <= targetCap) break;
        const auto now = std::chrono::steady_clock::now();
        if (now >= deadline) break;
        const auto rem = std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now);
        std::unique_lock<std::mutex> lk(out->waitMu);
        out->waiting.store(true, std::memory_order_relaxed);
        out->waitCv.wait_for(lk, std::min(rem, std::chrono::milliseconds(4)));
        out->waiting.store(false, std::memory_order_relaxed);
    }

    if (out->disconnected.load(std::memory_order_acquire)) {
        return static_cast<jint>(oboe::Result::ErrorDisconnected);
    }
    if (out->closing.load(std::memory_order_acquire)) return 0;

    const uint64_t r = out->readIdx.load(std::memory_order_acquire);
    const uint64_t w = out->writeIdx.load(std::memory_order_relaxed);
    const int freeFrames = kOutputRingFrames - static_cast<int>(w - r);
    const int toWrite = std::min(frames, std::max(0, freeFrames));
    if (toWrite <= 0) return 0;

    env->GetShortArrayRegion(in, 0, toWrite * ch, h->buffer.data());
    for (int i = 0; i < toWrite; ++i) {
        const size_t idx = static_cast<size_t>((w + i) & kOutputRingMask) * ch;
        for (int c = 0; c < ch; ++c) {
            out->ring[idx + c] = h->buffer[i * ch + c];
        }
    }
    out->writeIdx.store(w + toWrite, std::memory_order_release);
    return toWrite;
}

JNIEXPORT jint JNICALL Java_com_owlmic_media_audio_Oboe_queuedFrames(JNIEnv *, jclass, jlong handle) {
    Holder *h = holder(handle);
    if (h == nullptr || !h->output) return 0;
    const uint64_t r = h->output->readIdx.load(std::memory_order_acquire);
    const uint64_t w = h->output->writeIdx.load(std::memory_order_acquire);
    return static_cast<jint>(w - r);
}

JNIEXPORT void JNICALL Java_com_owlmic_media_audio_Oboe_close(JNIEnv *, jclass, jlong handle) {
    Holder *h = holder(handle);
    if (h->output) {
        h->output->closing.store(true, std::memory_order_release);
        {
            std::unique_lock<std::mutex> lk(h->output->waitMu);
        }
        h->output->waitCv.notify_all();
    }
    h->stream->requestStop();
    h->stream->close();
    delete h;
}

}  // extern "C"
