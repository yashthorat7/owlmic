// Owlmic PC Win32 Flyout Panel Script - Bento 6-Square Layout
// Conforms to 1 Big Playground on top + 5 Square Buttons in a single line below

document.addEventListener('DOMContentLoaded', () => {
    const root = document.getElementById('view-pc');
    if (!root) return;

    const devicePc = root.querySelector('.device-pc') || root;
    const deviceMobile = document.querySelector('.js-device-mobile');

    // 1. Top Playground Elements
    const playgroundTile = root.querySelector('.pc-playground-tile');
    const cameraCanvas = root.querySelector('.js-pc-camera-canvas');
    const cameraWatermark = root.querySelector('.js-pc-camera-watermark');

    // Waveform Dock inside Playground
    const waveformDock = root.querySelector('.js-pc-waveform-dock');
    const waveformContainer = root.querySelector('.js-pc-waveform');

    // 2. Bottom 5 Square Bento Buttons (Order: Help, Cam, Mic, Speaker, Wi-Fi)
    const btnHelp = root.querySelector('.js-pc-btn-help');
    const btnCam = root.querySelector('.js-pc-btn-cam');
    const btnMic = root.querySelector('.js-pc-btn-mic');
    const btnSpk = root.querySelector('.js-pc-btn-spk');
    const btnWifi = root.querySelector('.js-pc-btn-wifi');
    const indicatorIcon = root.querySelector('.js-pc-indicator-icon');

    // Register canvas with central renderer
    if (cameraCanvas) {
        cameraCanvas.width = 340;
        cameraCanvas.height = 340;
        OwlmicStore.registerCameraCanvas(cameraCanvas);
    }

    // Build Waveform Bars in Playground (25 bars)
    const BAR_COUNT = 25;
    const waveBars = [];
    if (waveformContainer) {
        waveformContainer.innerHTML = '';
        for (let i = 0; i < BAR_COUNT; i++) {
            const bar = document.createElement('div');
            bar.className = 'wave-bar';
            waveformContainer.appendChild(bar);
            waveBars.push(bar);
        }
    }

    // Dynamic Waveform Animation Loop
    let waveTime = 0;
    function animatePcWaveform() {
        waveTime += 0.055;
        const state = OwlmicStore.getState();
        const isMicLive = state.mic;

        if (isMicLive && waveBars.length > 0) {
            const isDocked = waveformDock && waveformDock.classList.contains('is-docked');
            // Min height equals width so quiescent capsules are circles; maxHeight reduced by 50% (64px center, 16px docked)
            const minHeight = isDocked ? 3 : 4;
            const maxHeight = isDocked ? 16 : 64;
            const audioLevel = state.audioLevel || 0.06;
            const centerIdx = (BAR_COUNT - 1) / 2;

            for (let i = 0; i < BAR_COUNT; i++) {
                const distFromCenter = Math.abs(i - centerIdx) / centerIdx;
                const envelope = Math.max(0.2, Math.cos(distFromCenter * Math.PI * 0.46));

                const wave1 = Math.sin(waveTime * 5.2 + i * 0.52);
                const wave2 = Math.sin(waveTime * 9.1 + i * 1.1) * 0.5;
                const wave3 = Math.cos(waveTime * 14.0 + i * 0.28) * 0.25;
                const dynamicMotion = (wave1 + wave2 + wave3) / 1.75;

                const speechBoost = Math.max(0, audioLevel * 2.2);
                const heightFraction = Math.max(0.08, (speechBoost * 0.95 + (dynamicMotion + 1) * 0.14 * (0.2 + speechBoost))) * envelope;
                const barHeight = minHeight + (maxHeight - minHeight) * Math.min(1.0, heightFraction);

                waveBars[i].style.height = `${barHeight.toFixed(1)}px`;
            }
        }

        requestAnimationFrame(animatePcWaveform);
    }
    requestAnimationFrame(animatePcWaveform);

    // PC Playground Drawer elements
    const pcDrawer = root.querySelector('.js-pc-playground-drawer');
    const pcDrawerScreenWifi = root.querySelector('.js-pc-drawer-screen-wifi');
    const pcDrawerScreenHelp = root.querySelector('.js-pc-drawer-screen-help');
    const pcDrawerBackBtns = root.querySelectorAll('.js-pc-drawer-back-btn');
    const pcConnTypeBtns = root.querySelectorAll('.js-pc-conn-type-btn');
    const pcDrawerPhoneName = root.querySelector('.js-pc-drawer-phone-name');

    function openPcWifiDrawer() {
        if (!pcDrawer) return;
        if (btnHelp) btnHelp.classList.remove('is-on');
        if (btnWifi) btnWifi.classList.add('is-on');
        if (pcDrawerScreenWifi) pcDrawerScreenWifi.style.display = 'flex';
        if (pcDrawerScreenHelp) pcDrawerScreenHelp.style.display = 'none';
        pcDrawer.classList.add('is-open');
    }

    function openPcHelpDrawer() {
        if (!pcDrawer) return;
        if (btnWifi) btnWifi.classList.remove('is-on');
        if (btnHelp) btnHelp.classList.add('is-on');
        if (pcDrawerScreenHelp) pcDrawerScreenHelp.style.display = 'flex';
        if (pcDrawerScreenWifi) pcDrawerScreenWifi.style.display = 'none';
        pcDrawer.classList.add('is-open');
    }

    function closePcDrawer() {
        if (!pcDrawer) return;
        pcDrawer.classList.remove('is-open');
        if (btnWifi) btnWifi.classList.remove('is-on');
        if (btnHelp) btnHelp.classList.remove('is-on');
    }

    // Event listeners - Main Screen Bottom Buttons
    if (btnHelp) {
        btnHelp.addEventListener('click', () => {
            const isHelpOpen = pcDrawer && pcDrawer.classList.contains('is-open') && 
                               pcDrawerScreenHelp && pcDrawerScreenHelp.style.display !== 'none';
            if (isHelpOpen) {
                closePcDrawer();
            } else {
                openPcHelpDrawer();
            }
        });
    }

    if (btnCam) {
        btnCam.addEventListener('click', () => {
            OwlmicStore.toggleCamera();
        });
    }

    if (btnMic) {
        btnMic.addEventListener('click', () => {
            OwlmicStore.toggleMic();
        });
    }

    if (btnSpk) {
        btnSpk.addEventListener('click', () => {
            OwlmicStore.toggleSpeaker();
        });
    }

    if (btnWifi) {
        btnWifi.addEventListener('click', () => {
            const isWifiOpen = pcDrawer && pcDrawer.classList.contains('is-open') && 
                               pcDrawerScreenWifi && pcDrawerScreenWifi.style.display !== 'none';
            if (isWifiOpen) {
                closePcDrawer();
            } else {
                openPcWifiDrawer();
            }
        });
    }

    pcDrawerBackBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            closePcDrawer();
        });
    });

    pcConnTypeBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            btn.classList.toggle('is-on');
        });
    });

    // State subscriber
    OwlmicStore.subscribe((state) => {
        const isCameraLive = state.camera;
        const isMicLive = state.mic;

        // Button Active Inversion States (Matches mobile behavior)
        if (btnCam) btnCam.classList.toggle('is-on', Boolean(state.camera));
        if (btnMic) btnMic.classList.toggle('is-on', Boolean(state.mic));
        if (btnSpk) btnSpk.classList.toggle('is-on', Boolean(state.speaker));

        // Mirror mode on playground
        if (playgroundTile) {
            playgroundTile.classList.toggle('is-mirror', state.cameraMirror);
        }

        // Camera Preview Canvas
        if (cameraCanvas) {
            cameraCanvas.style.display = isCameraLive ? 'block' : 'none';
        }

        // Watermark: hidden when either camera or mic is live
        if (cameraWatermark) {
            cameraWatermark.style.display = (isCameraLive || isMicLive) ? 'none' : 'flex';
        }

        // Waveform Dock in PC Playground (Matches mobile 100%)
        if (waveformDock) {
            if (isMicLive) {
                const wasActive = waveformDock.classList.contains('is-active');
                const wasDocked = waveformDock.classList.contains('is-docked');
                const targetDocked = isCameraLive;

                if (wasActive && wasDocked !== targetDocked) {
                    waveformDock.classList.add('is-animating');
                } else {
                    waveformDock.classList.remove('is-animating');
                }

                waveformDock.classList.add('is-active');
                if (targetDocked) {
                    waveformDock.classList.add('is-docked');
                    waveformDock.classList.remove('is-centered');
                } else {
                    waveformDock.classList.add('is-centered');
                    waveformDock.classList.remove('is-docked');
                }
            } else {
                waveformDock.classList.remove('is-animating');
                waveformDock.classList.remove('is-active');
                waveformDock.classList.remove('is-centered');
                waveformDock.classList.remove('is-docked');
            }
        }

        if (pcDrawerPhoneName) pcDrawerPhoneName.textContent = state.phoneName;

        // Wi-Fi Indicator Icon (Never dimmed, follows button state)
        if (indicatorIcon) {
            if (btnWifi && btnWifi.classList.contains('is-on')) {
                indicatorIcon.style.color = '#000000';
            } else {
                indicatorIcon.style.color = '#E6E6E6';
            }
            indicatorIcon.style.opacity = '1';
        }
    });
});
