// Owlmic PC Win32 Flyout Panel Script - Bento 6-Square Layout
// Conforms to 1 Big Playground on top + 5 Square Buttons in a single line below

document.addEventListener('DOMContentLoaded', () => {
    const root = document.getElementById('view-pc');
    if (!root) return;

    const devicePc = root.querySelector('.device-pc') || root;
    const deviceMobile = document.querySelector('.js-device-mobile');

    // 1. Top Playground Elements
    const playgroundTile = root.querySelector('.pc-playground-tile');
    const cameraBackdropCanvas = root.querySelector('.js-pc-camera-backdrop');
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

    // Screens (Main Bento vs Approval Gate)
    const pcScreenMain = root.querySelector('.js-pc-screen-main');
    const pcScreenGate = root.querySelector('.js-pc-screen-gate');
    const pcGateTitle = root.querySelector('.js-pc-gate-title');
    const pcGateCode = root.querySelector('.js-pc-gate-code');
    const btnGateAllow = root.querySelector('.js-pc-gate-allow');
    const btnGateDeny = root.querySelector('.js-pc-gate-deny');

    // Repair Banner & Disconnected Overlay
    const pcRepairBanner = root.querySelector('.js-pc-repair-banner');
    const btnRepair = root.querySelector('.js-pc-repair-btn');
    const repairText = root.querySelector('.js-pc-repair-text');
    const pcDisconnectedOverlay = root.querySelector('.js-pc-disconnected-overlay');

    // Register canvases with central renderer
    if (cameraBackdropCanvas) {
        cameraBackdropCanvas.width = 340;
        cameraBackdropCanvas.height = 340;
        OwlmicStore.registerCameraCanvas(cameraBackdropCanvas);
    }
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
    const pcDrawerScreenMic = root.querySelector('.js-pc-drawer-screen-mic');
    const pcConnTypeBtns = root.querySelectorAll('.js-pc-conn-type-btn');
    const pcDrawerPhoneName = root.querySelector('.js-pc-drawer-phone-name');
    const pcReconnectBtn = root.querySelector('.js-pc-footer-btn-reconnect');
    const pcReconnectSpinIcon = root.querySelector('.js-pc-reconnect-spin');

    // Floating Camera Controls (Eye, Mirror, Flip)
    const pcCameraControls = root.querySelector('.js-pc-camera-controls');
    const btnWatch = root.querySelector('.js-pc-btn-watch');
    const iconWatchOn = btnWatch ? btnWatch.querySelector('.icon-watch-on') : null;
    const iconWatchOff = btnWatch ? btnWatch.querySelector('.icon-watch-off') : null;
    const btnMirror = root.querySelector('.js-pc-btn-mirror');
    const btnFlip = root.querySelector('.js-pc-btn-flip');

    // Floating Mic Settings Button
    const btnMicSettings = root.querySelector('.js-pc-btn-mic-settings');

    // Mic Drawer Controls (Sliders & Switches)
    const sliderNoise = root.querySelector('.js-pc-slider-noise');
    const noiseVal = root.querySelector('.js-pc-noise-val');
    const sliderVolume = root.querySelector('.js-pc-slider-volume');
    const volumeVal = root.querySelector('.js-pc-volume-val');
    const toggleRowIsolation = root.querySelector('.js-pc-toggle-row-isolation');
    const switchIsolation = root.querySelector('.js-pc-switch-isolation');
    const toggleRowAutolevel = root.querySelector('.js-pc-toggle-row-autolevel');
    const switchAutolevel = root.querySelector('.js-pc-switch-autolevel');

    // PC Capsule Elements inside PC Waveform Dock
    const pcCapsuleDrawer = root.querySelector('.js-pc-capsule-drawer');
    const pcCapsuleWaveArea = root.querySelector('.js-pc-capsule-wave-area');
    const pcCapsuleSliderNoise = root.querySelector('.js-pc-capsule-slider-noise');
    const pcCapsuleNoiseVal = root.querySelector('.js-pc-capsule-noise-val');
    const pcCapsuleSliderVolume = root.querySelector('.js-pc-capsule-slider-volume');
    const pcCapsuleVolumeVal = root.querySelector('.js-pc-capsule-volume-val');
    const pcCapsuleToggleIsolation = root.querySelector('.js-pc-capsule-toggle-isolation');
    const pcCapsuleSwitchIsolation = root.querySelector('.js-pc-capsule-switch-isolation');
    const pcCapsuleToggleAutolevel = root.querySelector('.js-pc-capsule-toggle-autolevel');
    const pcCapsuleSwitchAutolevel = root.querySelector('.js-pc-capsule-switch-autolevel');

    function openPcWifiDrawer() {
        if (!pcDrawer) return;
        if (btnHelp) btnHelp.classList.remove('is-on');
        if (btnMicSettings) btnMicSettings.classList.remove('is-on');
        if (btnWifi) btnWifi.classList.add('is-on');
        if (pcDrawerScreenWifi) pcDrawerScreenWifi.style.display = 'flex';
        if (pcDrawerScreenHelp) pcDrawerScreenHelp.style.display = 'none';
        if (pcDrawerScreenMic) pcDrawerScreenMic.style.display = 'none';
        pcDrawer.classList.add('is-open');
    }

    function openPcHelpDrawer() {
        if (!pcDrawer) return;
        if (btnWifi) btnWifi.classList.remove('is-on');
        if (btnMicSettings) btnMicSettings.classList.remove('is-on');
        if (btnHelp) btnHelp.classList.add('is-on');
        if (pcDrawerScreenHelp) pcDrawerScreenHelp.style.display = 'flex';
        if (pcDrawerScreenWifi) pcDrawerScreenWifi.style.display = 'none';
        if (pcDrawerScreenMic) pcDrawerScreenMic.style.display = 'none';
        pcDrawer.classList.add('is-open');
    }

    function openPcMicDrawer() {
        if (!pcDrawer) return;
        if (btnWifi) btnWifi.classList.remove('is-on');
        if (btnHelp) btnHelp.classList.remove('is-on');
        if (btnMicSettings) btnMicSettings.classList.add('is-on');
        if (pcDrawerScreenMic) pcDrawerScreenMic.style.display = 'flex';
        if (pcDrawerScreenWifi) pcDrawerScreenWifi.style.display = 'none';
        if (pcDrawerScreenHelp) pcDrawerScreenHelp.style.display = 'none';
        pcDrawer.classList.add('is-open');
    }

    function closePcDrawer() {
        if (!pcDrawer) return;
        pcDrawer.classList.remove('is-open');
        if (btnWifi) btnWifi.classList.remove('is-on');
        if (btnHelp) btnHelp.classList.remove('is-on');
        if (btnMicSettings) btnMicSettings.classList.remove('is-on');
    }

    // Event listeners - Camera Controls
    if (btnWatch) {
        btnWatch.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleWatching();
        });
    }

    if (btnMirror) {
        btnMirror.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleMirror();
        });
    }

    if (btnFlip) {
        btnFlip.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.flipLens();
        });
    }

    // Event listeners - Approval Gate & Repair Actions
    if (btnGateAllow) {
        btnGateAllow.addEventListener('click', () => {
            OwlmicStore.allowApproval();
        });
    }

    if (btnGateDeny) {
        btnGateDeny.addEventListener('click', () => {
            OwlmicStore.denyApproval();
        });
    }

    if (btnRepair) {
        btnRepair.addEventListener('click', () => {
            OwlmicStore.repairNow();
        });
    }

    // Event listeners - Mic Settings Button & Waveform Click
    if (btnMicSettings) {
        btnMicSettings.addEventListener('click', (e) => {
            e.stopPropagation();
            const isMicOpen = pcDrawer && pcDrawer.classList.contains('is-open') &&
                              pcDrawerScreenMic && pcDrawerScreenMic.style.display !== 'none';
            if (isMicOpen) {
                closePcDrawer();
            } else {
                openPcMicDrawer();
            }
        });
    }

    if (waveformDock) {
        waveformDock.addEventListener('click', (e) => {
            const state = OwlmicStore.getState();
            if (state.camera && state.mic) {
                e.stopPropagation();
                waveformDock.classList.toggle('is-expanded');
            }
        });
    }

    if (pcCapsuleDrawer) {
        pcCapsuleDrawer.addEventListener('click', (e) => {
            e.stopPropagation();
        });
    }

    if (pcCapsuleSliderNoise) {
        pcCapsuleSliderNoise.addEventListener('input', (e) => {
            OwlmicStore.setNoiseReduction(e.target.value);
        });
    }

    if (pcCapsuleSliderVolume) {
        pcCapsuleSliderVolume.addEventListener('input', (e) => {
            OwlmicStore.setVolume(e.target.value);
        });
    }

    if (pcCapsuleToggleIsolation) {
        pcCapsuleToggleIsolation.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleIsolation();
        });
    }

    if (pcCapsuleToggleAutolevel) {
        pcCapsuleToggleAutolevel.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleAutoLevel();
        });
    }

    const pcPlaygroundTile = root.querySelector('.js-pc-camera-tile');
    if (pcPlaygroundTile) {
        pcPlaygroundTile.addEventListener('click', () => {
            if (waveformDock && waveformDock.classList.contains('is-expanded')) {
                waveformDock.classList.remove('is-expanded');
            }
        });
    }

    // Mic Drawer Sliders & Switches
    if (sliderNoise) {
        sliderNoise.addEventListener('input', (e) => {
            OwlmicStore.setNoiseReduction(e.target.value);
        });
    }

    if (sliderVolume) {
        sliderVolume.addEventListener('input', (e) => {
            OwlmicStore.setVolume(e.target.value);
        });
    }

    if (toggleRowIsolation) {
        toggleRowIsolation.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleIsolation();
        });
    }

    if (toggleRowAutolevel) {
        toggleRowAutolevel.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleAutoLevel();
        });
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

    if (pcReconnectBtn) {
        pcReconnectBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            if (pcReconnectSpinIcon) {
                pcReconnectSpinIcon.classList.remove('is-spinning');
                void pcReconnectSpinIcon.offsetWidth;
                pcReconnectSpinIcon.classList.add('is-spinning');
            }
            OwlmicStore.reconnect();
        });
    }

    pcConnTypeBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const type = btn.getAttribute('data-type');
            if (type) {
                OwlmicStore.toggleLink(type);
            }
        });
    });

    // Drawer PC expand/collapse rows (Matching Compose Drawers.kt accordion behavior)
    const pcDrawerRows = root.querySelectorAll('.js-pc-drawer-pc-toggle');
    pcDrawerRows.forEach(toggle => {
        toggle.addEventListener('click', (e) => {
            e.stopPropagation();
            const parent = toggle.closest('.drawer-pc-entry');
            if (!parent) return;
            const expand = parent.querySelector('.drawer-entry-expand');
            if (!expand) return;
            const isCurrentlyOpen = expand.style.display !== 'none';

            // Close all other rows
            root.querySelectorAll('.js-pc-drawer-screen-wifi .drawer-pc-entry').forEach(entry => {
                entry.classList.remove('is-open');
                const rowExpand = entry.querySelector('.drawer-entry-expand');
                if (rowExpand) rowExpand.style.display = 'none';
            });

            if (!isCurrentlyOpen) {
                expand.style.display = 'flex';
                parent.classList.add('is-open');
            }
        });
    });

    // Wire PC Connection Drawer Actions
    const pcActionBlock = root.querySelector('.js-pc-action-block');
    if (pcActionBlock) {
        pcActionBlock.addEventListener('click', (e) => {
            e.stopPropagation();
            const isBlocked = pcActionBlock.textContent === 'Unblock';
            pcActionBlock.textContent = isBlocked ? 'Block' : 'Unblock';
            pcActionBlock.style.color = isBlocked ? '#FF453A' : '#8E8E93';
        });
    }

    const pcActionConnect = root.querySelector('.js-pc-action-connect');
    if (pcActionConnect) {
        pcActionConnect.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.reconnect();
        });
    }

    const pcActionRemove = root.querySelector('.js-pc-action-remove');
    if (pcActionRemove) {
        pcActionRemove.addEventListener('click', (e) => {
            e.stopPropagation();
            const parentEntry = pcActionRemove.closest('.drawer-pc-entry');
            if (parentEntry) parentEntry.style.display = 'none';
        });
    }

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
            playgroundTile.classList.toggle('is-mirror', Boolean(state.cameraMirror));
        }

        // Camera Preview Canvas, Backdrop & Eye State
        if (cameraBackdropCanvas) {
            cameraBackdropCanvas.style.display = (isCameraLive && state.cameraWatching) ? 'block' : 'none';
        }
        if (cameraCanvas) {
            cameraCanvas.style.display = (isCameraLive && state.cameraWatching) ? 'block' : 'none';
        }

        // Floating Camera Controls Visibility & State
        if (pcCameraControls) {
            pcCameraControls.style.display = isCameraLive ? 'flex' : 'none';
        }
        if (btnWatch) {
            btnWatch.classList.toggle('is-off', !state.cameraWatching);
            if (iconWatchOn) iconWatchOn.style.display = state.cameraWatching ? 'block' : 'none';
            if (iconWatchOff) iconWatchOff.style.display = state.cameraWatching ? 'none' : 'block';
        }
        if (btnMirror) {
            btnMirror.classList.toggle('is-on', Boolean(state.cameraMirror));
        }
        if (btnFlip) {
            btnFlip.title = `Flip Lens: ${state.cameraLens}`;
        }

        // Mic Settings Button Visibility
        if (btnMicSettings) {
            // Visible when mic is on and camera is off (when camera is on, waveform dock in capsule opens drawer)
            btnMicSettings.style.display = (isMicLive && !isCameraLive) ? 'inline-flex' : 'none';
        }

        // Mic Settings Drawer values synchronization
        if (sliderNoise) {
            sliderNoise.value = state.micNoiseReduction ?? 5;
        }
        if (noiseVal) {
            const nr = state.micNoiseReduction ?? 5;
            noiseVal.textContent = nr === 0 ? 'Off' : String(nr);
        }

        if (sliderVolume) {
            sliderVolume.value = state.micVolume ?? 0;
        }
        if (volumeVal) {
            const v = state.micVolume ?? 0;
            volumeVal.textContent = (v > 0 ? `+${v}` : `${v}`) + ' dB';
        }

        if (switchIsolation) {
            switchIsolation.classList.toggle('is-on', Boolean(state.micIsolation));
        }
        if (switchAutolevel) {
            switchAutolevel.classList.toggle('is-on', Boolean(state.micAutoLevel));
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
                    waveformDock.title = 'Click to open microphone settings';
                    // When camera is on, close top mic drawer if it was open
                    if (pcDrawer && pcDrawer.classList.contains('is-open') &&
                        pcDrawerScreenMic && pcDrawerScreenMic.style.display !== 'none') {
                        closePcDrawer();
                    }
                } else {
                    waveformDock.classList.add('is-centered');
                    waveformDock.classList.remove('is-docked', 'is-expanded');
                    waveformDock.title = '';
                }
            } else {
                waveformDock.classList.remove('is-animating', 'is-active', 'is-centered', 'is-docked', 'is-expanded');
                waveformDock.title = '';
            }
        }

        // PC Capsule Controls Synchronization
        if (pcCapsuleSliderNoise) pcCapsuleSliderNoise.value = state.micNoiseReduction ?? 5;
        if (pcCapsuleNoiseVal) {
            const nr = state.micNoiseReduction ?? 5;
            pcCapsuleNoiseVal.textContent = nr === 0 ? 'Off' : String(nr);
        }
        if (pcCapsuleSliderVolume) pcCapsuleSliderVolume.value = state.micVolume ?? 0;
        if (pcCapsuleVolumeVal) {
            const v = state.micVolume ?? 0;
            pcCapsuleVolumeVal.textContent = (v > 0 ? `+${v}` : `${v}`) + ' dB';
        }
        if (pcCapsuleSwitchIsolation) pcCapsuleSwitchIsolation.classList.toggle('is-on', Boolean(state.micIsolation));
        if (pcCapsuleSwitchAutolevel) pcCapsuleSwitchAutolevel.classList.toggle('is-on', Boolean(state.micAutoLevel));

        if (pcDrawerPhoneName) pcDrawerPhoneName.textContent = state.phoneName;

        // Screen switching: Gate vs Main
        if (pcScreenGate && pcScreenMain) {
            const isGate = state.pcScreen === 'gate' || state.connection === 'approving';
            if (isGate) {
                pcScreenGate.classList.add('active');
                pcScreenMain.classList.remove('active');
                if (pcGateTitle) pcGateTitle.textContent = `${state.phoneName} wants to connect`;
                if (pcGateCode) pcGateCode.textContent = state.code || '4821';
            } else {
                pcScreenGate.classList.remove('active');
                pcScreenMain.classList.add('active');
            }
        }

        // Repair Banner
        if (pcRepairBanner) {
            const isRepair = state.connection === 'repair' || state.repairing;
            pcRepairBanner.style.display = isRepair ? 'flex' : 'none';
            if (btnRepair) {
                btnRepair.textContent = state.repairing ? 'Repairing...' : 'Repair';
                btnRepair.disabled = Boolean(state.repairing);
            }
            if (repairText) {
                repairText.textContent = state.repairing ? 'Repairing audio/video bridges...' : 'Owlmic Mic needs a repair';
            }
        }


        // Wi-Fi Indicator Icon (Pulsing when approving/reconnecting, Alert when offline/repair)
        if (indicatorIcon && btnWifi) {
            btnWifi.classList.remove('is-pulsing', 'is-alert');
            if (state.connection === 'approving' || state.connection === 'reconnecting') {
                btnWifi.classList.add('is-pulsing');
            } else if (state.connection === 'not_connected') {
                btnWifi.classList.add('is-alert');
            } else if (state.connection === 'repair') {
                indicatorIcon.style.color = '#FFD60A';
                indicatorIcon.style.opacity = '1';
            } else {
                if (btnWifi.classList.contains('is-on')) {
                    indicatorIcon.style.color = '#000000';
                } else {
                    indicatorIcon.style.color = '#E6E6E6';
                }
                indicatorIcon.style.opacity = '1';
            }
        }

        // Connection links synchronization
        if (state.links) {
            pcConnTypeBtns.forEach(btn => {
                const type = btn.getAttribute('data-type');
                if (type && state.links[type] !== undefined) {
                    btn.classList.toggle('is-on', Boolean(state.links[type]));
                }
            });
        }
    });
});
