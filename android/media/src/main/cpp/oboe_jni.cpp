// JNI side of com.owlmic.media.audio.Oboe: the mic's input stream and the speaker's output stream.
// Both are used with blocking reads and writes on their own threads, so nothing here runs in a real-time callback.
#include <jni.h>
#include <oboe/Oboe.h>
#include <memory>
#include <vector>

namespace {

struct Holder {
    std::shared_ptr<oboe::AudioStream> stream;
    std::vector<int16_t> buffer;
};

constexpr int kSampleRate = 48000;
constexpr int kMaxFrames = 4800;

Holder *holder(jlong handle) { return reinterpret_cast<Holder *>(handle); }

jlong open(oboe::AudioStreamBuilder &builder, int channels) {
    std::shared_ptr<oboe::AudioStream> stream;
    if (builder.openStream(stream) != oboe::Result::OK) return 0;
    if (stream->requestStart() != oboe::Result::OK) {
        stream->close();
        return 0;
    }
    auto *h = new Holder{stream, std::vector<int16_t>(kMaxFrames * channels)};
    return reinterpret_cast<jlong>(h);
}

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
    return open(builder, 1);
}

// usage: 1 media, 2 voice communication. contentType: 1 speech, 2 music.
JNIEXPORT jlong JNICALL Java_com_owlmic_media_audio_Oboe_openOutput(JNIEnv *, jclass, jint channels, jint usage, jint contentType, jboolean lowLatency) {
    oboe::AudioStreamBuilder builder;
    builder.setDirection(oboe::Direction::Output)
        ->setSharingMode(oboe::SharingMode::Shared)
        ->setPerformanceMode(mode(lowLatency))
        ->setFormat(oboe::AudioFormat::I16)
        ->setChannelCount(channels)
        ->setSampleRate(kSampleRate)
        ->setSampleRateConversionQuality(oboe::SampleRateConversionQuality::Medium)
        ->setUsage(static_cast<oboe::Usage>(usage))
        ->setContentType(static_cast<oboe::ContentType>(contentType));
    return open(builder, channels);
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

// Returns frames written, or a negative oboe::Result.
JNIEXPORT jint JNICALL Java_com_owlmic_media_audio_Oboe_write(JNIEnv *env, jclass, jlong handle, jshortArray in, jint frames, jint timeoutMs) {
    Holder *h = holder(handle);
    if (frames > kMaxFrames) frames = kMaxFrames;
    env->GetShortArrayRegion(in, 0, frames * h->stream->getChannelCount(), h->buffer.data());
    auto result = h->stream->write(h->buffer.data(), frames, static_cast<int64_t>(timeoutMs) * 1000000);
    if (!result) return static_cast<jint>(result.error());
    return result.value();
}

JNIEXPORT void JNICALL Java_com_owlmic_media_audio_Oboe_close(JNIEnv *, jclass, jlong handle) {
    Holder *h = holder(handle);
    h->stream->requestStop();
    h->stream->close();
    delete h;
}

}  // extern "C"
