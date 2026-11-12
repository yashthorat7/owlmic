// Owlmic Prototype Sandbox - Core State Store & Engine
// Shared by Mobile, Mobile Horizontal, and PC views

(function() {
    'use strict';

    // Application state matching Owlmic architecture
    const state = {
        connection: 'connected', // 'connected' | 'approving' | 'reconnecting' | 'not_connected' | 'repair'
        link: 'wifi',            // 'wifi' | 'usb_debugging' | 'usb_tethering' | 'bluetooth'
        phoneName: 'Pixel 8',
        pcName: 'Workstation',
        code: '4821',
        camera: false,
        cameraQuality: '1080p',
        cameraFps: 30,
        cameraLens: 'Back',      // 'Back' | 'Front'
        cameraFraming: 'Fill',   // 'Fill' | 'Fit'
        cameraMirror: false,
        cameraOrientation: 'Auto',
        mic: false,
        micBoost: 'Off',
        micNoise: 'Phone',
        audioLevel: 0.0,
        speaker: false,
        speakerOutput: 'Phone speaker',
        pcScreen: 'main',        // 'main' | 'gate' | 'settings'
        mobileSettingsOpen: false,
        repairing: false,
        appPowered: true,        // When false, simulates mobile app closed/killed
        pcPowered: true,         // When false, simulates PC tray app closed/quit
        pairedPhones: [
            { id: 'pixel-8', name: 'Pixel 8', blocked: false, current: true }
        ],
        addresses: ['192.168.1.15', '10.0.0.42']
    };

    const listeners = new Set();

    function notify() {
        listeners.forEach(fn => fn(state));
    }

    // Audio level meter simulation loop
    let audioTimer = null;
    let targetAudio = 0;
    function updateAudio() {
        if (state.appPowered && state.pcPowered && state.mic && state.connection === 'connected') {
            if (Math.random() < 0.15) {
                targetAudio = Math.random() * 0.85 + 0.1;
            } else if (Math.random() < 0.2) {
                targetAudio = 0.05;
            }
            state.audioLevel += (targetAudio - state.audioLevel) * 0.25;
        } else {
            state.audioLevel = 0.0;
        }
        notify();
        requestAnimationFrame(updateAudio);
    }
    requestAnimationFrame(updateAudio);

    // Camera canvas renderer
    const registeredCanvases = new Set();

    function registerCameraCanvas(canvas) {
        registeredCanvases.add(canvas);
    }

    function unregisterCameraCanvas(canvas) {
        registeredCanvases.delete(canvas);
    }

    let frameCount = 0;
    function renderCameras() {
        frameCount++;
        if (state.appPowered && state.pcPowered && state.camera && (state.connection === 'connected' || state.connection === 'repair')) {
            const time = frameCount * 0.03;
            registeredCanvases.forEach(canvas => {
                if (!canvas.isConnected) return;
                const ctx = canvas.getContext('2d');
                const w = canvas.width;
                const h = canvas.height;

                // Pure transparent background - no solid fill, no grid dots, no sharp edges
                ctx.clearRect(0, 0, w, h);

                // Check device mobile orientation
                const isPhoneCanvas = canvas.classList.contains('js-mobile-camera-canvas');
                const deviceMobile = document.querySelector('.js-device-mobile');
                const isPhoneLandscape = deviceMobile && deviceMobile.classList.contains('is-landscape');

                ctx.save();
                ctx.translate(w * 0.5, h * 0.5);
                if (isPhoneCanvas && isPhoneLandscape) {
                    ctx.rotate(Math.PI / 2); // 90deg counter-rotation for rotating phone chassis
                }

                // Smooth organic breathing and micro-drift
                const drift = Math.sin(time * 0.45) * 2.0;
                const breath = 1.0 + Math.sin(time * 0.8) * 0.008;
                const scale = (Math.min(w, h) / 360.0) * (isPhoneLandscape ? 1.08 : 1.0);
                
                // Prominent, well-proportioned portrait framing (larger realistic face)
                const R = 66.0 * scale * breath;
                const headCy = -44.0 * scale + drift;

                // 1. Soft atmospheric glow behind silhouette
                const glow = ctx.createRadialGradient(0, headCy, R * 0.25, 0, headCy + R * 0.5, R * 2.8);
                glow.addColorStop(0, 'rgba(255, 255, 255, 0.09)');
                glow.addColorStop(0.5, 'rgba(255, 255, 255, 0.025)');
                glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
                ctx.fillStyle = glow;
                ctx.beginPath();
                ctx.arc(0, headCy + R * 0.5, R * 2.8, 0, Math.PI * 2);
                ctx.fill();

                // 2. Torso / Shoulders (Natural, uncropped portrait bust)
                const neckW = 28.0 * scale;
                const neckTop = headCy + R * 0.74;
                const neckBot = headCy + R * 1.28;
                const shoulderW = R * 2.35;
                const bodyBot = h * 0.54;

                ctx.fillStyle = '#1a1c26';
                ctx.beginPath();
                ctx.moveTo(-neckW, neckBot);
                // Left trapezius and shoulder slope
                ctx.bezierCurveTo(-neckW - 32 * scale, neckBot + 12 * scale, -shoulderW * 0.65, neckBot + 32 * scale, -shoulderW, neckBot + 65 * scale);
                ctx.bezierCurveTo(-shoulderW * 1.15, neckBot + 95 * scale, -w * 0.52, bodyBot * 0.8, -w * 0.55, bodyBot);
                ctx.lineTo(w * 0.55, bodyBot);
                // Right shoulder slope
                ctx.bezierCurveTo(w * 0.52, bodyBot * 0.8, shoulderW * 1.15, neckBot + 95 * scale, shoulderW, neckBot + 65 * scale);
                ctx.bezierCurveTo(shoulderW * 0.65, neckBot + 32 * scale, neckW + 32 * scale, neckBot + 12 * scale, neckW, neckBot);
                ctx.closePath();
                ctx.fill();

                // Subtle shoulder rim light highlight
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.16)';
                ctx.lineWidth = 1.5 * scale;
                ctx.beginPath();
                ctx.moveTo(-shoulderW * 0.95, neckBot + 62 * scale);
                ctx.bezierCurveTo(-shoulderW * 0.65, neckBot + 30 * scale, -neckW - 32 * scale, neckBot + 10 * scale, -neckW, neckBot);
                ctx.lineTo(neckW, neckBot);
                ctx.bezierCurveTo(neckW + 32 * scale, neckBot + 10 * scale, shoulderW * 0.65, neckBot + 30 * scale, shoulderW * 0.95, neckBot + 62 * scale);
                ctx.stroke();

                // 3. Neck
                ctx.fillStyle = '#1e212e';
                ctx.beginPath();
                ctx.moveTo(-neckW, neckTop);
                ctx.lineTo(neckW, neckTop);
                ctx.lineTo(neckW * 1.05, neckBot + 2);
                ctx.lineTo(-neckW * 1.05, neckBot + 2);
                ctx.closePath();
                ctx.fill();

                // 4. Head (Sculpted portrait head with natural cheek & chin contours)
                ctx.fillStyle = '#222636';
                ctx.beginPath();
                // Cranium top
                ctx.arc(0, headCy - R * 0.16, R * 0.86, Math.PI, 0, false);
                // Right cheek & jawline tapering to chin
                ctx.bezierCurveTo(R * 0.92, headCy + R * 0.35, R * 0.58, headCy + R * 0.84, 0, headCy + R * 0.98);
                // Left jawline & cheek tapering from chin
                ctx.bezierCurveTo(-R * 0.58, headCy + R * 0.84, -R * 0.92, headCy + R * 0.35, -R * 0.86, headCy - R * 0.16);
                ctx.closePath();
                ctx.fill();

                // Head top rim highlight
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.22)';
                ctx.lineWidth = 1.5 * scale;
                ctx.beginPath();
                ctx.arc(0, headCy - R * 0.16, R * 0.86, Math.PI * 1.15, Math.PI * 1.85, false);
                ctx.stroke();

                // Subtle collar neckline arc
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.11)';
                ctx.lineWidth = 1.5 * scale;
                ctx.beginPath();
                ctx.arc(0, neckBot + 16 * scale, 34 * scale, 0.2 * Math.PI, 0.8 * Math.PI, false);
                ctx.stroke();

                ctx.restore();
            });
        } else {
            registeredCanvases.forEach(canvas => {
                if (!canvas.isConnected) return;
                const ctx = canvas.getContext('2d');
                ctx.clearRect(0, 0, canvas.width, canvas.height);
            });
        }
        requestAnimationFrame(renderCameras);
    }
    requestAnimationFrame(renderCameras);

    // Global store actions
    window.OwlmicStore = {
        getState: () => ({ ...state }),
        subscribe: (fn) => {
            listeners.add(fn);
            fn(state);
            return () => listeners.delete(fn);
        },
        toggleCamera: () => {
            if (!state.appPowered || !state.pcPowered) {
                state.appPowered = true;
                state.pcPowered = true;
            }
            if (state.connection !== 'connected') {
                state.connection = 'connected';
                state.pcScreen = 'main';
            }
            state.camera = !state.camera;
            notify();
        },
        toggleMic: () => {
            if (!state.appPowered || !state.pcPowered) {
                state.appPowered = true;
                state.pcPowered = true;
            }
            if (state.connection !== 'connected') {
                state.connection = 'connected';
                state.pcScreen = 'main';
            }
            state.mic = !state.mic;
            notify();
        },
        toggleSpeaker: () => {
            if (!state.appPowered || !state.pcPowered) {
                state.appPowered = true;
                state.pcPowered = true;
            }
            if (state.connection !== 'connected') {
                state.connection = 'connected';
                state.pcScreen = 'main';
            }
            state.speaker = !state.speaker;
            notify();
        },
        flipLens: () => {
            state.cameraLens = state.cameraLens === 'Back' ? 'Front' : 'Back';
            notify();
        },
        cycleFraming: () => {
            state.cameraFraming = state.cameraFraming === 'Fill' ? 'Fit' : 'Fill';
            notify();
        },
        toggleMirror: () => {
            state.cameraMirror = !state.cameraMirror;
            notify();
        },
        cycleQuality: () => {
            const qualities = ['Auto', '1080p', '720p'];
            const idx = qualities.indexOf(state.cameraQuality);
            state.cameraQuality = qualities[(idx + 1) % qualities.length];
            notify();
        },
        cycleFps: () => {
            const fpsList = [24, 30, 60];
            const idx = fpsList.indexOf(state.cameraFps);
            state.cameraFps = fpsList[(idx + 1) % fpsList.length];
            notify();
        },
        cycleNoiseReduction: () => {
            const opts = ['Phone', 'Off'];
            state.micNoise = state.micNoise === 'Phone' ? 'Off' : 'Phone';
            notify();
        },
        cycleBoost: () => {
            const opts = ['Off', '+6 dB', '+12 dB'];
            const idx = opts.indexOf(state.micBoost);
            state.micBoost = opts[(idx + 1) % opts.length];
            notify();
        },
        setConnectionState: (cState) => {
            state.connection = cState;
            if (cState === 'approving') {
                state.pcScreen = 'gate';
                state.camera = false;
                state.mic = false;
                state.speaker = false;
            } else if (cState === 'not_connected') {
                state.pcScreen = 'main';
                state.camera = false;
                state.mic = false;
                state.speaker = false;
            } else if (cState === 'repair') {
                state.pcScreen = 'main';
            } else {
                state.pcScreen = 'main';
            }
            notify();
        },
        setLinkKind: (link) => {
            state.link = link;
            notify();
        },
        cycleLinkKind: () => {
            const links = ['wifi', 'usb_debugging', 'usb_tethering', 'bluetooth'];
            const idx = links.indexOf(state.link);
            state.link = links[(idx + 1) % links.length];
            notify();
        },
        toggleAppPower: () => {
            state.appPowered = !state.appPowered;
            if (!state.appPowered) {
                // App disturbed / shut down
                if (state.connection === 'connected') {
                    state.connection = 'reconnecting';
                }
            } else {
                // App turned back on
                if (state.pcPowered && state.connection === 'reconnecting') {
                    state.connection = 'connected';
                }
            }
            notify();
        },
        togglePcPower: () => {
            state.pcPowered = !state.pcPowered;
            if (!state.pcPowered) {
                // PC tray app closed
                if (state.connection === 'connected') {
                    state.connection = 'reconnecting';
                }
            } else {
                // PC tray app restored
                if (state.appPowered && state.connection === 'reconnecting') {
                    state.connection = 'connected';
                }
            }
            notify();
        },
        disturbConnection: () => {
            // Cycle disturbance: connected -> reconnecting -> not_connected -> approving -> repair -> connected
            const transitions = {
                'connected': 'reconnecting',
                'reconnecting': 'not_connected',
                'not_connected': 'approving',
                'approving': 'repair',
                'repair': 'connected'
            };
            const next = transitions[state.connection] || 'connected';
            window.OwlmicStore.setConnectionState(next);
        },
        openMobileSettings: (open) => {
            state.mobileSettingsOpen = open;
            notify();
        },
        setPcScreen: (scr) => {
            state.pcScreen = scr;
            notify();
        },
        allowApproval: () => {
            state.connection = 'connected';
            state.pcScreen = 'main';
            notify();
        },
        denyApproval: () => {
            state.connection = 'not_connected';
            state.pcScreen = 'main';
            notify();
        },
        repairNow: () => {
            state.repairing = true;
            notify();
            setTimeout(() => {
                state.repairing = false;
                state.connection = 'connected';
                notify();
            }, 1200);
        },
        registerCameraCanvas,
        unregisterCameraCanvas
    };
})();
