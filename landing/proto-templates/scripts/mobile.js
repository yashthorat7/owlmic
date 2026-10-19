// Owlmic Mobile Portrait View Script

document.addEventListener('DOMContentLoaded', () => {
    const root = document.querySelector('.phone-portrait-layout');
    if (!root) return;

    // Status Bar & Notification Shade Elements
    const clockEl = document.querySelector('.js-status-clock');
    const statusBar = document.querySelector('.js-phone-status-bar');
    const notifShade = document.querySelector('.js-phone-notif-shade');
    const notifDismiss = document.querySelector('.js-notif-dismiss');
    const notifTitle = document.querySelector('.js-notif-title');
    const notifDesc = document.querySelector('.js-notif-desc');
    const notifMuteBtn = document.querySelector('.js-notif-action-mute');
    const notifStatusBtn = document.querySelector('.js-notif-action-status');

    // Live clock update
    function updateClock() {
        if (!clockEl) return;
        const now = new Date();
        const hours = now.getHours();
        const minutes = String(now.getMinutes()).padStart(2, '0');
        clockEl.textContent = `${hours}:${minutes}`;
    }
    updateClock();
    setInterval(updateClock, 10000);

    // Notification shade interactive drag-down and toggle feature
    if (statusBar && notifShade) {
        let isDragging = false;
        let startY = 0;
        let currentDrag = 0;
        let hasMoved = false;

        function getIsLandscape() {
            const deviceMobile = document.querySelector('.js-device-mobile');
            return deviceMobile && deviceMobile.classList.contains('is-landscape');
        }

        function onPointerDown(e) {
            if (e.target.closest('.notif-card') && !e.target.closest('.notif-card-header')) return;
            isDragging = true;
            hasMoved = false;
            startY = e.clientY;
            currentDrag = 0;
            statusBar.style.cursor = 'grabbing';
            document.addEventListener('pointermove', onPointerMove);
            document.addEventListener('pointerup', onPointerUp);
        }

        function onPointerMove(e) {
            if (!isDragging) return;
            const dy = e.clientY - startY;
            if (Math.abs(dy) > 4) {
                hasMoved = true;
            }

            const isLandscape = getIsLandscape();
            const isOpen = notifShade.classList.contains('is-open');

            if (!isOpen) {
                currentDrag = Math.max(0, Math.min(dy, 120));
                if (!isLandscape) {
                    notifShade.style.transition = 'none';
                    notifShade.style.transform = `translateY(${Math.min(currentDrag, 100)}px)`;
                    notifShade.style.opacity = String(Math.min(1, currentDrag / 30));
                    notifShade.style.pointerEvents = 'none';
                } else {
                    notifShade.style.transition = 'none';
                    const pct = Math.max(0, Math.min(100, (currentDrag / 70) * 100));
                    notifShade.style.transform = `translate(${140 - (pct * 1.4)}%, -50%) rotate(90deg)`;
                    notifShade.style.opacity = String(Math.min(1, currentDrag / 30));
                    notifShade.style.pointerEvents = 'none';
                }
            } else {
                if (dy < 0) {
                    currentDrag = dy;
                    if (!isLandscape) {
                        notifShade.style.transition = 'none';
                        notifShade.style.transform = `translateY(${Math.max(-100, 38 + dy)}px)`;
                        notifShade.style.opacity = String(Math.max(0, 1 + (dy / 80)));
                    } else {
                        notifShade.style.transition = 'none';
                        const offsetPct = Math.min(140, Math.abs(dy) * 1.5);
                        notifShade.style.transform = `translate(${offsetPct}%, -50%) rotate(90deg)`;
                        notifShade.style.opacity = String(Math.max(0, 1 + (dy / 80)));
                    }
                }
            }
        }

        function onPointerUp(e) {
            if (!isDragging) return;
            isDragging = false;
            statusBar.style.cursor = 'grab';
            document.removeEventListener('pointermove', onPointerMove);
            document.removeEventListener('pointerup', onPointerUp);

            notifShade.style.transition = '';
            notifShade.style.transform = '';
            notifShade.style.opacity = '';
            notifShade.style.pointerEvents = '';

            const isOpen = notifShade.classList.contains('is-open');

            if (!hasMoved) {
                // Click/tap toggle
                notifShade.classList.toggle('is-open');
            } else {
                if (!isOpen) {
                    if (currentDrag > 28) {
                        notifShade.classList.add('is-open');
                    } else {
                        notifShade.classList.remove('is-open');
                    }
                } else {
                    if (currentDrag < -20) {
                        notifShade.classList.remove('is-open');
                    } else {
                        notifShade.classList.add('is-open');
                    }
                }
            }
        }

        statusBar.addEventListener('pointerdown', onPointerDown);

        const notifCard = notifShade.querySelector('.js-notif-card');
        if (notifCard) {
            notifCard.addEventListener('pointerdown', (e) => {
                if (e.target.closest('button')) return;
                onPointerDown(e);
            });
        }
    }

    if (notifDismiss && notifShade) {
        notifDismiss.addEventListener('click', (e) => {
            e.stopPropagation();
            notifShade.classList.remove('is-open');
            if (helpBtn) helpBtn.classList.remove('is-on');
        });

        document.addEventListener('pointerdown', (e) => {
            if (!notifShade.classList.contains('is-open')) return;
            if (!e.target.closest('.js-phone-notif-shade') && !e.target.closest('.js-phone-status-bar') && !e.target.closest('.js-mobile-help-btn')) {
                notifShade.classList.remove('is-open');
                if (helpBtn) helpBtn.classList.remove('is-on');
            }
        });
    }

    if (notifMuteBtn) {
        notifMuteBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleMic();
        });
    }

    if (notifStatusBtn) {
        notifStatusBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const current = OwlmicStore.getState().connection;
            if (current === 'connected') OwlmicStore.setConnectionState('approving');
            else if (current === 'approving') OwlmicStore.setConnectionState('reconnecting');
            else OwlmicStore.setConnectionState('connected');
        });
    }

    // Elements
    const cameraTile = root.querySelector('.js-mobile-camera');
    const cameraCanvas = root.querySelector('.js-mobile-camera-canvas');
    const cameraOffFace = root.querySelector('.js-mobile-camera-off');
    const cameraOverlay = root.querySelector('.js-mobile-camera-overlay');
    const flipBtn = root.querySelector('.js-mobile-flip');
    const cameraCaption = root.querySelector('.js-mobile-camera-caption');
    const playgroundWatermark = root.querySelector('.js-playground-watermark');

    const micTile = root.querySelector('.js-mobile-mic');
    const micState = root.querySelector('.js-mobile-mic-state');
    const micMeter = root.querySelector('.js-mobile-mic-meter');

    const speakerTile = root.querySelector('.js-mobile-speaker');
    const speakerState = root.querySelector('.js-mobile-speaker-state');
    const speakerDetail = root.querySelector('.js-mobile-speaker-detail');

    const statusTile = root.querySelector('.js-mobile-status-tile');
    const statusPc = root.querySelector('.js-mobile-status-pc');
    const statusText = root.querySelector('.js-mobile-status-text');
    const indicatorIcon = root.querySelector('.js-mobile-indicator-icon');
    const helpBtn = root.querySelector('.js-mobile-help-btn');

    // Register canvas with central renderer
    if (cameraCanvas) {
        cameraCanvas.width = 375;
        cameraCanvas.height = 440;
        OwlmicStore.registerCameraCanvas(cameraCanvas);
    }

    // Event listeners
    if (cameraTile) {
        cameraTile.addEventListener('click', (e) => {
            if (e.target.closest('.camera-flip-btn')) return;
            OwlmicStore.toggleCamera();
        });
    }

    if (flipBtn) {
        flipBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.flipLens();
        });
    }

    if (micTile) {
        micTile.addEventListener('click', () => {
            OwlmicStore.toggleMic();
        });
    }

    if (speakerTile) {
        speakerTile.addEventListener('click', () => {
            OwlmicStore.toggleSpeaker();
        });
    }

    // Playground Drawer elements (Connection Settings, Help & Mic Controls)
    const playgroundDrawer = root.querySelector('.js-playground-drawer');
    const drawerScreenWifi = root.querySelector('.js-drawer-screen-wifi');
    const drawerScreenHelp = root.querySelector('.js-drawer-screen-help');
    const drawerScreenMic = root.querySelector('.js-drawer-screen-mic');
    const drawerBackBtns = root.querySelectorAll('.js-drawer-back-btn');
    const connTypeBtns = root.querySelectorAll('.js-conn-type-btn');
    const drawerPcName = root.querySelector('.js-drawer-pc-name');

    // Floating Camera Controls (Eye, Mirror, Flip)
    const mobileCameraControls = root.querySelector('.js-mobile-camera-controls');
    const mobileBtnWatch = root.querySelector('.js-mobile-btn-watch');
    const mobileIconWatchOn = mobileBtnWatch ? mobileBtnWatch.querySelector('.icon-watch-on') : null;
    const mobileIconWatchOff = mobileBtnWatch ? mobileBtnWatch.querySelector('.icon-watch-off') : null;
    const mobileBtnMirror = root.querySelector('.js-mobile-btn-mirror');
    const mobileBtnFlip = root.querySelector('.js-mobile-btn-flip');

    // Floating Mic Settings Button
    const mobileBtnMicSettings = root.querySelector('.js-mobile-btn-mic-settings');

    // Mobile Mic Drawer Controls
    const mobileSliderNoise = root.querySelector('.js-mobile-slider-noise');
    const mobileNoiseVal = root.querySelector('.js-mobile-noise-val');
    const mobileSliderVolume = root.querySelector('.js-mobile-slider-volume');
    const mobileVolumeVal = root.querySelector('.js-mobile-volume-val');
    const mobileToggleRowIsolation = root.querySelector('.js-mobile-toggle-row-isolation');
    const mobileSwitchIsolation = root.querySelector('.js-mobile-switch-isolation');
    const mobileToggleRowAutolevel = root.querySelector('.js-mobile-toggle-row-autolevel');
    const mobileSwitchAutolevel = root.querySelector('.js-mobile-switch-autolevel');

    function openWifiDrawer() {
        if (!playgroundDrawer) return;
        if (helpBtn) helpBtn.classList.remove('is-on');
        if (mobileBtnMicSettings) mobileBtnMicSettings.classList.remove('is-on');
        if (statusTile) statusTile.classList.add('is-on');
        if (drawerScreenWifi) drawerScreenWifi.style.display = 'flex';
        if (drawerScreenHelp) drawerScreenHelp.style.display = 'none';
        if (drawerScreenMic) drawerScreenMic.style.display = 'none';
        playgroundDrawer.classList.add('is-open');
    }

    function openHelpDrawer() {
        if (!playgroundDrawer) return;
        if (statusTile) statusTile.classList.remove('is-on');
        if (mobileBtnMicSettings) mobileBtnMicSettings.classList.remove('is-on');
        if (helpBtn) helpBtn.classList.add('is-on');
        if (drawerScreenHelp) drawerScreenHelp.style.display = 'flex';
        if (drawerScreenWifi) drawerScreenWifi.style.display = 'none';
        if (drawerScreenMic) drawerScreenMic.style.display = 'none';
        playgroundDrawer.classList.add('is-open');
    }

    function openMobileMicDrawer() {
        if (!playgroundDrawer) return;
        if (statusTile) statusTile.classList.remove('is-on');
        if (helpBtn) helpBtn.classList.remove('is-on');
        if (mobileBtnMicSettings) mobileBtnMicSettings.classList.add('is-on');
        if (drawerScreenMic) drawerScreenMic.style.display = 'flex';
        if (drawerScreenWifi) drawerScreenWifi.style.display = 'none';
        if (drawerScreenHelp) drawerScreenHelp.style.display = 'none';
        playgroundDrawer.classList.add('is-open');
    }

    function closeDrawer() {
        if (!playgroundDrawer) return;
        playgroundDrawer.classList.remove('is-open');
        if (statusTile) statusTile.classList.remove('is-on');
        if (helpBtn) helpBtn.classList.remove('is-on');
        if (mobileBtnMicSettings) mobileBtnMicSettings.classList.remove('is-on');
    }

    // Camera Controls Event Listeners
    if (mobileBtnWatch) {
        mobileBtnWatch.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleWatching();
        });
    }

    if (mobileBtnMirror) {
        mobileBtnMirror.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleMirror();
        });
    }

    if (mobileBtnFlip) {
        mobileBtnFlip.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.flipLens();
        });
    }

    // Mic Settings Button & Waveform Dock Click
    if (mobileBtnMicSettings) {
        mobileBtnMicSettings.addEventListener('click', (e) => {
            e.stopPropagation();
            const isMicOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                              drawerScreenMic && drawerScreenMic.style.display !== 'none';
            if (isMicOpen) {
                closeDrawer();
            } else {
                openMobileMicDrawer();
            }
        });
    }

    if (waveformDock) {
        waveformDock.addEventListener('click', (e) => {
            const state = OwlmicStore.getState();
            if (state.mic) {
                e.stopPropagation();
                const isMicOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                                  drawerScreenMic && drawerScreenMic.style.display !== 'none';
                if (isMicOpen) {
                    closeDrawer();
                } else {
                    openMobileMicDrawer();
                }
            }
        });
    }

    // Sliders & Switches Event Listeners
    if (mobileSliderNoise) {
        mobileSliderNoise.addEventListener('input', (e) => {
            OwlmicStore.setNoiseReduction(e.target.value);
        });
    }

    if (mobileSliderVolume) {
        mobileSliderVolume.addEventListener('input', (e) => {
            OwlmicStore.setVolume(e.target.value);
        });
    }

    if (mobileToggleRowIsolation) {
        mobileToggleRowIsolation.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleIsolation();
        });
    }

    if (mobileToggleRowAutolevel) {
        mobileToggleRowAutolevel.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleAutoLevel();
        });
    }

    // Status/WiFi tile tap: toggles Wi-Fi drawer (Connection Settings)
    if (statusTile) {
        statusTile.addEventListener('click', (e) => {
            e.stopPropagation();
            if (notifShade) notifShade.classList.remove('is-open');
            const isWifiOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') && 
                               drawerScreenWifi && drawerScreenWifi.style.display !== 'none';
            if (isWifiOpen) {
                closeDrawer();
            } else {
                openWifiDrawer();
            }
        });
    }

    // Help tile tap: toggles Help drawer
    if (helpBtn) {
        helpBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            if (notifShade) notifShade.classList.remove('is-open');
            const isHelpOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') && 
                               drawerScreenHelp && drawerScreenHelp.style.display !== 'none';
            if (isHelpOpen) {
                closeDrawer();
            } else {
                openHelpDrawer();
            }
        });
    }

    // Drawer back button
    drawerBackBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            closeDrawer();
        });
    });

    // Connection types toggleable buttons (all start toggled on, click to toggle on/off)
    connTypeBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            btn.classList.toggle('is-on');
        });
    });

    // Playground Audio Waveform Dock elements
    const waveformDock = root.querySelector('.js-playground-waveform-dock');
    const waveformContainer = root.querySelector('.js-playground-waveform');

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
    function animateWaveform() {
        waveTime += 0.055;
        const state = OwlmicStore.getState();
        const isMicLive = state.mic && state.connection === 'connected';

        if (isMicLive && waveBars.length > 0) {
            const isDocked = waveformDock && waveformDock.classList.contains('is-docked');
            // Min height equals width so quiescent capsules are circles; maxHeight reduced by 50% (74px center, 19px docked)
            const minHeight = isDocked ? 3.5 : 5;
            const maxHeight = isDocked ? 19 : 74;
            const audioLevel = state.audioLevel || 0.06;
            const centerIdx = (BAR_COUNT - 1) / 2;

            for (let i = 0; i < BAR_COUNT; i++) {
                const distFromCenter = Math.abs(i - centerIdx) / centerIdx; // 0 to 1
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

        requestAnimationFrame(animateWaveform);
    }
    requestAnimationFrame(animateWaveform);

    // State subscriber
    OwlmicStore.subscribe((state) => {
        const isCameraLive = state.camera;
        const isMicLive = state.mic;

        // Camera button & preview
        if (cameraTile) {
            if (isCameraLive) {
                cameraTile.classList.add('is-on');
                if (cameraCanvas) cameraCanvas.style.display = state.cameraWatching ? 'block' : 'none';
                if (cameraOverlay) cameraOverlay.style.display = 'flex';
                if (cameraOffFace) cameraOffFace.style.display = 'none';
                if (cameraCaption) cameraCaption.textContent = `${state.cameraQuality} · ${state.cameraFps} fps`;
            } else {
                cameraTile.classList.remove('is-on');
                if (cameraCanvas) cameraCanvas.style.display = 'none';
                if (cameraOverlay) cameraOverlay.style.display = 'none';
                if (cameraOffFace) cameraOffFace.style.display = 'flex';
            }
        }

        // Floating Camera Controls State & Visibility
        if (mobileCameraControls) {
            mobileCameraControls.style.display = isCameraLive ? 'flex' : 'none';
        }
        if (mobileBtnWatch) {
            mobileBtnWatch.classList.toggle('is-off', !state.cameraWatching);
            if (mobileIconWatchOn) mobileIconWatchOn.style.display = state.cameraWatching ? 'block' : 'none';
            if (mobileIconWatchOff) mobileIconWatchOff.style.display = state.cameraWatching ? 'none' : 'block';
        }
        if (mobileBtnMirror) {
            mobileBtnMirror.classList.toggle('is-on', Boolean(state.cameraMirror));
        }
        if (mobileBtnFlip) {
            mobileBtnFlip.title = `Flip Lens: ${state.cameraLens}`;
        }

        // Floating Mic Settings Button
        if (mobileBtnMicSettings) {
            mobileBtnMicSettings.style.display = (isMicLive && !isCameraLive) ? 'inline-flex' : 'none';
        }

        // Mic Drawer Values Synchronization
        if (mobileSliderNoise) mobileSliderNoise.value = state.micNoiseReduction ?? 5;
        if (mobileNoiseVal) {
            const nr = state.micNoiseReduction ?? 5;
            mobileNoiseVal.textContent = nr === 0 ? 'Off' : String(nr);
        }
        if (mobileSliderVolume) mobileSliderVolume.value = state.micVolume ?? 0;
        if (mobileVolumeVal) {
            const v = state.micVolume ?? 0;
            mobileVolumeVal.textContent = (v > 0 ? `+${v}` : `${v}`) + ' dB';
        }
        if (mobileSwitchIsolation) mobileSwitchIsolation.classList.toggle('is-on', Boolean(state.micIsolation));
        if (mobileSwitchAutolevel) mobileSwitchAutolevel.classList.toggle('is-on', Boolean(state.micAutoLevel));

        // Owl Watermark visibility:
        // "whem mic ison the wol in teh pa lygroudn removes"
        // Hidden when either Mic is ON or Camera is ON
        if (playgroundWatermark) {
            playgroundWatermark.style.display = (isMicLive || isCameraLive) ? 'none' : 'flex';
        }

        // Waveform Dock positioning & background rectangle:
        // When mic is ON and camera is OFF -> centered without bg capsule
        // When both are ON -> moves smoothly to bottom center inside 12px bento rectangle
        // Appears instantly on toggle without entrance animation or rotation lag
        if (waveformDock) {
            if (isMicLive) {
                const wasActive = waveformDock.classList.contains('is-active');
                const wasDocked = waveformDock.classList.contains('is-docked');
                const targetDocked = isCameraLive;

                // Only animate position when dock was ALREADY active and is switching places
                if (wasActive && wasDocked !== targetDocked) {
                    waveformDock.classList.add('is-animating');
                } else {
                    waveformDock.classList.remove('is-animating');
                }

                waveformDock.classList.add('is-active');
                if (targetDocked) {
                    waveformDock.classList.remove('is-centered');
                    waveformDock.classList.add('is-docked');
                } else {
                    waveformDock.classList.remove('is-docked');
                    waveformDock.classList.add('is-centered');
                }
            } else {
                waveformDock.classList.remove('is-animating', 'is-active', 'is-centered', 'is-docked');
            }
        }

        // Mic button
        if (micTile) {
            if (state.mic) {
                micTile.classList.add('is-on');
            } else {
                micTile.classList.remove('is-on');
            }
            if (micState) micState.textContent = state.mic ? 'On' : 'Off';
            if (micMeter) {
                micMeter.style.display = state.mic ? 'block' : 'none';
                micMeter.style.width = Math.min(100, Math.max(0, state.audioLevel * 100)) + '%';
            }
        }

        // Speaker button
        if (speakerTile) {
            if (state.speaker) {
                speakerTile.classList.add('is-on');
            } else {
                speakerTile.classList.remove('is-on');
            }
            if (speakerState) speakerState.textContent = state.speaker ? 'On' : 'Off';
            if (speakerDetail) {
                speakerDetail.style.display = state.speaker ? 'block' : 'none';
                speakerDetail.textContent = state.speakerOutput;
            }
        }

        if (statusPc) statusPc.textContent = state.pcName;
        if (drawerPcName) drawerPcName.textContent = state.pcName;
        if (statusText) statusText.textContent = state.connection === 'connected' ? 'Connected' : state.connection;

        // Notification card content updates
        if (notifTitle && notifDesc) {
            if (state.connection !== 'connected') {
                notifTitle.textContent = state.connection === 'approving' 
                    ? 'Security: Approval Required' 
                    : state.connection === 'reconnecting' 
                    ? 'Reconnecting to PC...' 
                    : 'Owlmic Service Disconnected';
                notifDesc.textContent = state.connection === 'approving'
                    ? `PC '${state.pcName}' is requesting connection code ${state.code}`
                    : `Link: ${state.link.replace('_', ' ')} · Tap to retry`;
                if (notifMuteBtn) notifMuteBtn.textContent = 'Retry Link';
            } else if (state.mic && state.camera) {
                notifTitle.textContent = 'Mic & Camera Streaming';
                notifDesc.textContent = `Streaming 48kHz audio + ${state.cameraQuality} video to ${state.pcName}`;
                if (notifMuteBtn) notifMuteBtn.textContent = 'Mute Mic';
            } else if (state.mic) {
                notifTitle.textContent = 'Microphone Streaming Active';
                notifDesc.textContent = `Streaming 48kHz PCM audio to ${state.pcName} over Wi-Fi`;
                if (notifMuteBtn) notifMuteBtn.textContent = 'Mute Mic';
            } else if (state.camera) {
                notifTitle.textContent = 'Virtual Camera Streaming';
                notifDesc.textContent = `Streaming ${state.cameraQuality} ${state.cameraFps}fps video to ${state.pcName}`;
                if (notifMuteBtn) notifMuteBtn.textContent = 'Start Mic';
            } else {
                notifTitle.textContent = 'Owlmic Service Standby';
                notifDesc.textContent = `Connected to ${state.pcName} · Ready to stream`;
                if (notifMuteBtn) notifMuteBtn.textContent = 'Start Mic';
            }
        }
    });
});
