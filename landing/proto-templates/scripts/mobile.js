// Owlmic Mobile Android View Script
// Conforms strictly to android/app/src/main/java/com/owlmic/ui/MainScreen.kt, Drawers.kt, Controls.kt, Waveform.kt, SettingsSheet.kt

document.addEventListener('DOMContentLoaded', () => {
    const root = document.querySelector('.device-mobile') || document.querySelector('.phone-portrait-layout');
    if (!root) return;

    // --- 1. Top Status Bar Live Clock ---
    const liveTimeEl = root.querySelector('.js-status-live-time');
    function updateClock() {
        if (!liveTimeEl) return;
        const now = new Date();
        const hours = now.getHours();
        const minutes = now.getMinutes().toString().padStart(2, '0');
        liveTimeEl.textContent = `${hours}:${minutes}`;
    }
    updateClock();
    setInterval(updateClock, 10000);

    // --- 2. Element Lookups ---
    // Bottom Bar Tiles
    const cameraTile = root.querySelector('.js-mobile-camera');
    const micTile = root.querySelector('.js-mobile-mic');
    const speakerTile = root.querySelector('.js-mobile-speaker');

    // Camera Canvas & Floating Controls
    const cameraCanvas = root.querySelector('.js-mobile-camera-canvas');
    const playgroundWatermark = root.querySelector('.js-playground-watermark');
    const mobileCameraControls = root.querySelector('.js-mobile-camera-controls');
    const mobileBtnWatch = root.querySelector('.js-mobile-btn-watch');
    const mobileIconWatchOn = mobileBtnWatch ? mobileBtnWatch.querySelector('.icon-watch-on') : null;
    const mobileIconWatchOff = mobileBtnWatch ? mobileBtnWatch.querySelector('.icon-watch-off') : null;
    const mobileBtnMirror = root.querySelector('.js-mobile-btn-mirror');
    const mobileBtnFlip = root.querySelector('.js-mobile-btn-flip');

    // Mic Floating Button & Settings
    const mobileBtnMicSettings = root.querySelector('.js-mobile-btn-mic-settings');
    const mobileSliderNoise = root.querySelector('.js-mobile-slider-noise');
    const mobileNoiseVal = root.querySelector('.js-mobile-noise-val');
    const mobileSliderVolume = root.querySelector('.js-mobile-slider-volume');
    const mobileVolumeVal = root.querySelector('.js-mobile-volume-val');
    const mobileToggleRowIsolation = root.querySelector('.js-mobile-toggle-row-isolation');
    const mobileSwitchIsolation = root.querySelector('.js-mobile-switch-isolation');
    const mobileToggleRowAutolevel = root.querySelector('.js-mobile-toggle-row-autolevel');
    const mobileSwitchAutolevel = root.querySelector('.js-mobile-switch-autolevel');

    // Top Bar Partitions
    const helpBtn = root.querySelector('.js-mobile-help-btn');
    const statusTile = root.querySelector('.js-mobile-status-tile');
    const indicatorIcon = root.querySelector('.js-mobile-indicator-icon');

    // Status Strip
    const mobileStatusStrip = root.querySelector('.js-mobile-status-strip');
    const statusStripDevice = root.querySelector('.js-status-strip-device');
    const statusStripCode = root.querySelector('.js-status-strip-code');
    const statusStripHint = root.querySelector('.js-status-strip-hint');

    // Playground Drawers
    const playgroundDrawer = root.querySelector('.js-playground-drawer');
    const drawerScreenWifi = root.querySelector('.js-drawer-screen-wifi');
    const drawerScreenHelp = root.querySelector('.js-drawer-screen-help');
    const drawerScreenMic = root.querySelector('.js-drawer-screen-mic');
    const drawerPcName = root.querySelector('.js-drawer-pc-name');

    // Connection Drawer Footer
    const reconnectBtn = root.querySelector('.js-footer-btn-reconnect');
    const reconnectSpinIcon = root.querySelector('.js-reconnect-spin');
    const connTypeBtns = root.querySelectorAll('.js-conn-type-btn');

    // Help Drawer Subpages
    const helpSubpageMain = root.querySelector('.js-help-subpage-main');
    const helpSubpageHowto = root.querySelector('.js-help-subpage-howto');
    const helpSubpageCredits = root.querySelector('.js-help-subpage-credits');
    const btnHelpHowto = root.querySelector('.js-help-tile-howto');
    const btnHelpCredits = root.querySelector('.js-help-tile-credits');
    const btnHelpPrivacy = root.querySelector('.js-help-tile-privacy');
    const btnHelpRate = root.querySelector('.js-help-tile-rate');
    const btnHelpGithub = root.querySelector('.js-help-tile-github');
    const btnHelpDonate = root.querySelector('.js-help-tile-donate');
    const btnHowtoBack = root.querySelector('.js-howto-back-btn');
    const btnCreditsBack = root.querySelector('.js-credits-back-btn');

    // Full-Screen Settings Sheet (Compose SettingsSheet.kt)
    const settingsSheet = root.querySelector('.js-phone-settings-sheet');
    const settingsClose = root.querySelector('.js-settings-close');
    const sheetSliderNoise = root.querySelector('.js-sheet-slider-noise');
    const sheetNoiseVal = root.querySelector('.js-sheet-noise-val');
    const sheetSliderVolume = root.querySelector('.js-sheet-slider-volume');
    const sheetVolumeVal = root.querySelector('.js-sheet-volume-val');
    const sheetToggleIsolation = root.querySelector('.js-sheet-toggle-isolation');
    const sheetSwitchIsolation = root.querySelector('.js-sheet-switch-isolation');
    const sheetToggleAutolevel = root.querySelector('.js-sheet-toggle-autolevel');
    const sheetSwitchAutolevel = root.querySelector('.js-sheet-switch-autolevel');
    const sheetChipsLens = root.querySelectorAll('.js-sheet-chips-lens .chip-btn');
    const sheetChipsQuality = root.querySelectorAll('.js-sheet-chips-quality .chip-btn');
    const sheetChipsFps = root.querySelectorAll('.js-sheet-chips-fps .chip-btn');
    const sheetToggleMirror = root.querySelector('.js-sheet-toggle-mirror');
    const sheetSwitchMirror = root.querySelector('.js-sheet-switch-mirror');

    const sheetToggleUsbDeb = root.querySelector('.js-sheet-toggle-usb-debugging');
    const sheetSwitchUsbDeb = root.querySelector('.js-sheet-switch-usb-debugging');
    const sheetToggleWifi = root.querySelector('.js-sheet-toggle-wifi');
    const sheetSwitchWifi = root.querySelector('.js-sheet-switch-wifi');
    const sheetToggleUsbTeth = root.querySelector('.js-sheet-toggle-usb-tethering');
    const sheetSwitchUsbTeth = root.querySelector('.js-sheet-switch-usb-tethering');
    const sheetToggleBt = root.querySelector('.js-sheet-toggle-bluetooth');
    const sheetSwitchBt = root.querySelector('.js-sheet-switch-bluetooth');

    // --- 3. Playground Audio Waveform Dock & Capsule (Compose Playground.kt WaveHost) ---
    const waveformDock = root.querySelector('.js-playground-waveform-dock');
    const waveformContainer = root.querySelector('.js-playground-waveform');
    const capsuleDrawer = root.querySelector('.js-capsule-drawer');
    const capsuleWaveArea = root.querySelector('.js-capsule-wave-area');
    const capsuleSliderNoise = root.querySelector('.js-capsule-slider-noise');
    const capsuleNoiseVal = root.querySelector('.js-capsule-noise-val');
    const capsuleSliderVolume = root.querySelector('.js-capsule-slider-volume');
    const capsuleVolumeVal = root.querySelector('.js-capsule-volume-val');
    const capsuleToggleIsolation = root.querySelector('.js-capsule-toggle-isolation');
    const capsuleSwitchIsolation = root.querySelector('.js-capsule-switch-isolation');
    const capsuleToggleAutolevel = root.querySelector('.js-capsule-toggle-autolevel');
    const capsuleSwitchAutolevel = root.querySelector('.js-capsule-switch-autolevel');

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

    // --- 4. Central Camera Registration ---
    if (cameraCanvas) {
        cameraCanvas.width = 375;
        cameraCanvas.height = 440;
        OwlmicStore.registerCameraCanvas(cameraCanvas);
    }

    // --- 5. Drawer Navigation Helpers ---
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
        showHelpMain();
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

    function showHelpMain() {
        if (helpSubpageMain) helpSubpageMain.style.display = 'flex';
        if (helpSubpageHowto) helpSubpageHowto.style.display = 'none';
        if (helpSubpageCredits) helpSubpageCredits.style.display = 'none';
    }

    function showHelpHowto() {
        if (helpSubpageMain) helpSubpageMain.style.display = 'none';
        if (helpSubpageHowto) helpSubpageHowto.style.display = 'flex';
        if (helpSubpageCredits) helpSubpageCredits.style.display = 'none';
    }

    function showHelpCredits() {
        if (helpSubpageMain) helpSubpageMain.style.display = 'none';
        if (helpSubpageHowto) helpSubpageHowto.style.display = 'none';
        if (helpSubpageCredits) helpSubpageCredits.style.display = 'flex';
    }

    function openSettingsSheet(section) {
        if (!settingsSheet) return;
        closeDrawer();
        settingsSheet.style.display = 'flex';
        requestAnimationFrame(() => {
            settingsSheet.classList.add('is-open');
        });
        if (section) {
            const secEl = settingsSheet.querySelector(`[data-section="${section}"]`);
            if (secEl) {
                setTimeout(() => secEl.scrollIntoView({ behavior: 'smooth', block: 'start' }), 50);
            }
        }
    }

    function closeSettingsSheet() {
        if (!settingsSheet) return;
        settingsSheet.classList.remove('is-open');
        setTimeout(() => {
            settingsSheet.style.display = 'none';
        }, 320);
    }

    // --- 6. Long Press vs Tap Helper for Tiles ---
    function bindTilePress(element, onTap, onHold) {
        if (!element) return;
        let pressTimer = null;
        let isLongPress = false;

        const start = (e) => {
            if (e.button !== undefined && e.button !== 0) return;
            isLongPress = false;
            pressTimer = setTimeout(() => {
                isLongPress = true;
                if (onHold) onHold();
            }, 500);
        };

        const cancel = () => {
            if (pressTimer) {
                clearTimeout(pressTimer);
                pressTimer = null;
            }
        };

        const end = (e) => {
            cancel();
            if (!isLongPress && onTap) {
                onTap(e);
            }
        };

        element.addEventListener('pointerdown', start);
        element.addEventListener('pointerup', end);
        element.addEventListener('pointerleave', cancel);
        element.addEventListener('pointercancel', cancel);
    }

    // Bind Camera, Mic, Speaker with Long-press for SettingsSheet
    bindTilePress(cameraTile, () => OwlmicStore.toggleCamera(), () => openSettingsSheet('camera'));
    bindTilePress(micTile, () => OwlmicStore.toggleMic(), () => openSettingsSheet('mic'));
    bindTilePress(speakerTile, () => OwlmicStore.toggleSpeaker(), () => openSettingsSheet('speaker'));

    // Top Bar Partitions
    if (helpBtn) {
        helpBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const isHelpOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                               drawerScreenHelp && drawerScreenHelp.style.display !== 'none';
            if (isHelpOpen) closeDrawer();
            else openHelpDrawer();
        });
    }

    if (statusTile) {
        statusTile.addEventListener('click', (e) => {
            e.stopPropagation();
            const isWifiOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                               drawerScreenWifi && drawerScreenWifi.style.display !== 'none';
            if (isWifiOpen) closeDrawer();
            else openWifiDrawer();
        });
    }

    // Floating Camera Controls
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

    // Floating Mic Settings & Waveform Dock Click
    if (mobileBtnMicSettings) {
        mobileBtnMicSettings.addEventListener('click', (e) => {
            e.stopPropagation();
            const isMicOpen = playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                              drawerScreenMic && drawerScreenMic.style.display !== 'none';
            if (isMicOpen) closeDrawer();
            else openMobileMicDrawer();
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

    if (capsuleDrawer) {
        capsuleDrawer.addEventListener('click', (e) => {
            e.stopPropagation();
        });
    }

    if (capsuleSliderNoise) {
        capsuleSliderNoise.addEventListener('input', (e) => {
            OwlmicStore.setNoiseReduction(e.target.value);
        });
    }

    if (capsuleSliderVolume) {
        capsuleSliderVolume.addEventListener('input', (e) => {
            OwlmicStore.setVolume(e.target.value);
        });
    }

    if (capsuleToggleIsolation) {
        capsuleToggleIsolation.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleIsolation();
        });
    }

    if (capsuleToggleAutolevel) {
        capsuleToggleAutolevel.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleAutoLevel();
        });
    }

    const phonePlayground = root.querySelector('.js-phone-playground');
    if (phonePlayground) {
        phonePlayground.addEventListener('click', () => {
            if (waveformDock && waveformDock.classList.contains('is-expanded')) {
                waveformDock.classList.remove('is-expanded');
            }
        });
    }

    // Drawer Back Buttons
    drawerBackBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            closeDrawer();
        });
    });

    // Connection Drawer Footer Buttons
    if (reconnectBtn) {
        reconnectBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            if (reconnectSpinIcon) {
                reconnectSpinIcon.classList.remove('is-spinning');
                void reconnectSpinIcon.offsetWidth;
                reconnectSpinIcon.classList.add('is-spinning');
            }
            OwlmicStore.reconnect();
        });
    }

    connTypeBtns.forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const type = btn.getAttribute('data-type');
            if (type) {
                OwlmicStore.toggleLink(type);
            }
        });
    });

    // Drawer PC expand/collapse rows (Matching Compose Drawers.kt accordion behavior)
    const drawerPcEntries = root.querySelectorAll('.js-drawer-pc-toggle');
    drawerPcEntries.forEach(toggle => {
        toggle.addEventListener('click', (e) => {
            e.stopPropagation();
            const parent = toggle.closest('.drawer-pc-entry');
            if (!parent) return;
            const expand = parent.querySelector('.drawer-entry-expand');
            if (!expand) return;
            const isCurrentlyOpen = expand.style.display !== 'none';

            // Close all rows first
            root.querySelectorAll('.js-drawer-screen-wifi .drawer-pc-entry').forEach(entry => {
                entry.classList.remove('is-open');
                const rowExpand = entry.querySelector('.drawer-entry-expand');
                if (rowExpand) rowExpand.style.display = 'none';
            });

            // If it wasn't open, open it
            if (!isCurrentlyOpen) {
                expand.style.display = 'flex';
                parent.classList.add('is-open');
            }
        });
    });

    // Wire Mobile Connection Drawer Actions
    const mobileActionBlock = root.querySelector('.js-drawer-screen-wifi .js-action-block');
    if (mobileActionBlock) {
        mobileActionBlock.addEventListener('click', (e) => {
            e.stopPropagation();
            const isBlocked = mobileActionBlock.textContent === 'Unblock';
            mobileActionBlock.textContent = isBlocked ? 'Block' : 'Unblock';
            mobileActionBlock.style.color = isBlocked ? '#FF453A' : '#8E8E93';
        });
    }

    const mobileActionConnect = root.querySelector('.js-drawer-screen-wifi .js-action-connect');
    if (mobileActionConnect) {
        mobileActionConnect.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.reconnect();
        });
    }

    const mobileActionRemove = root.querySelector('.js-drawer-screen-wifi .js-action-remove');
    if (mobileActionRemove) {
        mobileActionRemove.addEventListener('click', (e) => {
            e.stopPropagation();
            const parentEntry = mobileActionRemove.closest('.drawer-pc-entry');
            if (parentEntry) parentEntry.style.display = 'none';
        });
    }

    // Setup & Help 2x3 Grid Handlers
    if (btnHelpHowto) {
        btnHelpHowto.addEventListener('click', (e) => {
            e.stopPropagation();
            showHelpHowto();
        });
    }

    if (btnHelpCredits) {
        btnHelpCredits.addEventListener('click', (e) => {
            e.stopPropagation();
            showHelpCredits();
        });
    }

    if (btnHowtoBack) {
        btnHowtoBack.addEventListener('click', (e) => {
            e.stopPropagation();
            showHelpMain();
        });
    }

    if (btnCreditsBack) {
        btnCreditsBack.addEventListener('click', (e) => {
            e.stopPropagation();
            showHelpMain();
        });
    }

    if (btnHelpPrivacy) {
        btnHelpPrivacy.addEventListener('click', () => {
            window.open('https://github.com/yashthorat7/owlmic/blob/main/docs/user/privacy.md', '_blank');
        });
    }

    if (btnHelpGithub) {
        btnHelpGithub.addEventListener('click', () => {
            window.open('https://github.com/yashthorat7/owlmic', '_blank');
        });
    }

    if (btnHelpRate) {
        btnHelpRate.addEventListener('click', () => {
            alert('Thank you for using Owlmic! ⭐⭐⭐⭐⭐');
        });
    }

    if (btnHelpDonate) {
        btnHelpDonate.addEventListener('click', () => {
            alert('Owlmic is 100% free and open-source! Donations support future development.');
        });
    }

    // Status Strip Action
    if (mobileStatusStrip) {
        mobileStatusStrip.addEventListener('click', () => {
            const state = OwlmicStore.getState();
            if (state.connection === 'repair') {
                OwlmicStore.repairNow();
            } else if (state.connection === 'not_connected') {
                OwlmicStore.setConnectionState('approving');
            }
        });
    }

    // Mic Drawer Sliders & Switches
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

    // Settings Sheet Events
    if (settingsClose) {
        settingsClose.addEventListener('click', closeSettingsSheet);
    }

    if (sheetSliderNoise) {
        sheetSliderNoise.addEventListener('input', (e) => {
            OwlmicStore.setNoiseReduction(e.target.value);
        });
    }

    if (sheetSliderVolume) {
        sheetSliderVolume.addEventListener('input', (e) => {
            OwlmicStore.setVolume(e.target.value);
        });
    }

    if (sheetToggleIsolation) {
        sheetToggleIsolation.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleIsolation();
        });
    }

    if (sheetToggleAutolevel) {
        sheetToggleAutolevel.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleAutoLevel();
        });
    }

    sheetChipsLens.forEach(chip => {
        chip.addEventListener('click', (e) => {
            e.stopPropagation();
            const val = chip.getAttribute('data-val');
            if (val) OwlmicStore.setLens(val);
        });
    });

    sheetChipsQuality.forEach(chip => {
        chip.addEventListener('click', (e) => {
            e.stopPropagation();
            const val = chip.getAttribute('data-val');
            if (val) OwlmicStore.setQuality(val);
        });
    });

    sheetChipsFps.forEach(chip => {
        chip.addEventListener('click', (e) => {
            e.stopPropagation();
            const val = chip.getAttribute('data-val');
            if (val) OwlmicStore.setFps(val);
        });
    });

    if (sheetToggleMirror) {
        sheetToggleMirror.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleMirror();
        });
    }

    if (sheetToggleUsbDeb) {
        sheetToggleUsbDeb.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleLink('usb_debugging');
        });
    }

    if (sheetToggleWifi) {
        sheetToggleWifi.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleLink('wifi');
        });
    }

    if (sheetToggleUsbTeth) {
        sheetToggleUsbTeth.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleLink('usb_tethering');
        });
    }

    if (sheetToggleBt) {
        sheetToggleBt.addEventListener('click', (e) => {
            e.stopPropagation();
            OwlmicStore.toggleLink('bluetooth');
        });
    }

    // --- 7. Central State Subscriber ---
    OwlmicStore.subscribe((state) => {
        const isCameraLive = state.camera;
        const isMicLive = state.mic;

        // Bottom Bar Tiles
        if (cameraTile) cameraTile.classList.toggle('is-on', Boolean(isCameraLive));
        if (micTile) micTile.classList.toggle('is-on', Boolean(isMicLive));
        if (speakerTile) speakerTile.classList.toggle('is-on', Boolean(state.speaker));

        // Camera Preview Canvas & Watermark
        if (cameraCanvas) {
            cameraCanvas.style.display = (isCameraLive && state.cameraWatching) ? 'block' : 'none';
        }
        if (playgroundWatermark) {
            playgroundWatermark.style.display = (isMicLive || isCameraLive) ? 'none' : 'flex';
        }

        // Floating Camera Controls
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

        // Waveform Dock State
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
                    waveformDock.classList.remove('is-centered');
                    waveformDock.classList.add('is-docked');
                    // When camera is on, close top mic drawer if it was opened
                    if (playgroundDrawer && playgroundDrawer.classList.contains('is-open') &&
                        drawerScreenMic && drawerScreenMic.style.display !== 'none') {
                        closeDrawer();
                    }
                } else {
                    waveformDock.classList.remove('is-docked', 'is-expanded', 'has-strip');
                    waveformDock.classList.add('is-centered');
                }
            } else {
                waveformDock.classList.remove('is-animating', 'is-active', 'is-centered', 'is-docked', 'is-expanded', 'has-strip');
            }
        }

        // Capsule Controls Synchronization
        if (capsuleSliderNoise) capsuleSliderNoise.value = state.micNoiseReduction ?? 5;
        if (capsuleNoiseVal) {
            const nr = state.micNoiseReduction ?? 5;
            capsuleNoiseVal.textContent = nr === 0 ? 'Off' : String(nr);
        }
        if (capsuleSliderVolume) capsuleSliderVolume.value = state.micVolume ?? 0;
        if (capsuleVolumeVal) {
            const v = state.micVolume ?? 0;
            capsuleVolumeVal.textContent = (v > 0 ? `+${v}` : `${v}`) + ' dB';
        }
        if (capsuleSwitchIsolation) capsuleSwitchIsolation.classList.toggle('is-on', Boolean(state.micIsolation));
        if (capsuleSwitchAutolevel) capsuleSwitchAutolevel.classList.toggle('is-on', Boolean(state.micAutoLevel));

        // Mic Drawer Values
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

        // Settings Sheet Values
        if (sheetSliderNoise) sheetSliderNoise.value = state.micNoiseReduction ?? 5;
        if (sheetNoiseVal) {
            const nr = state.micNoiseReduction ?? 5;
            sheetNoiseVal.textContent = nr === 0 ? 'Off' : String(nr);
        }
        if (sheetSliderVolume) sheetSliderVolume.value = state.micVolume ?? 0;
        if (sheetVolumeVal) {
            const v = state.micVolume ?? 0;
            sheetVolumeVal.textContent = (v > 0 ? `+${v}` : `${v}`) + ' dB';
        }
        if (sheetSwitchIsolation) sheetSwitchIsolation.classList.toggle('is-on', Boolean(state.micIsolation));
        if (sheetSwitchAutolevel) sheetSwitchAutolevel.classList.toggle('is-on', Boolean(state.micAutoLevel));

        sheetChipsLens.forEach(chip => {
            chip.classList.toggle('is-on', chip.getAttribute('data-val') === state.cameraLens);
        });
        sheetChipsQuality.forEach(chip => {
            chip.classList.toggle('is-on', chip.getAttribute('data-val') === state.cameraQuality);
        });
        sheetChipsFps.forEach(chip => {
            chip.classList.toggle('is-on', chip.getAttribute('data-val') === String(state.cameraFps));
        });
        if (sheetSwitchMirror) sheetSwitchMirror.classList.toggle('is-on', Boolean(state.cameraMirror));

        // Link Toggles Sync (Both Drawer Footer & Settings Sheet)
        if (state.links) {
            connTypeBtns.forEach(btn => {
                const type = btn.getAttribute('data-type');
                if (type) btn.classList.toggle('is-on', Boolean(state.links[type]));
            });
            if (sheetSwitchUsbDeb) sheetSwitchUsbDeb.classList.toggle('is-on', Boolean(state.links.usb_debugging));
            if (sheetSwitchWifi) sheetSwitchWifi.classList.toggle('is-on', Boolean(state.links.wifi));
            if (sheetSwitchUsbTeth) sheetSwitchUsbTeth.classList.toggle('is-on', Boolean(state.links.usb_tethering));
            if (sheetSwitchBt) sheetSwitchBt.classList.toggle('is-on', Boolean(state.links.bluetooth));
        }

        if (drawerPcName) drawerPcName.textContent = state.pcName;

        // Mobile Status Strip & Top Wi-Fi Indicator State
        if (mobileStatusStrip) {
            if (state.connection === 'approving') {
                mobileStatusStrip.style.display = 'flex';
                if (statusStripDevice) statusStripDevice.textContent = state.pcName;
                if (statusStripCode) {
                    statusStripCode.style.display = 'block';
                    statusStripCode.textContent = state.code || '4821';
                }
                if (statusStripHint) statusStripHint.textContent = 'Check your PC to approve';
                if (statusTile) {
                    statusTile.classList.add('is-pulsing');
                    statusTile.classList.remove('is-alert');
                }
            } else if (state.connection === 'reconnecting') {
                mobileStatusStrip.style.display = 'flex';
                if (statusStripDevice) statusStripDevice.textContent = 'Reconnecting to ' + state.pcName + '...';
                if (statusStripCode) statusStripCode.style.display = 'none';
                if (statusStripHint) statusStripHint.textContent = 'Hold tight while connection restores';
                if (statusTile) {
                    statusTile.classList.add('is-pulsing');
                    statusTile.classList.remove('is-alert');
                }
            } else if (state.connection === 'not_connected') {
                mobileStatusStrip.style.display = 'flex';
                if (statusStripDevice) statusStripDevice.textContent = 'Looking for your PC';
                if (statusStripCode) statusStripCode.style.display = 'none';
                if (statusStripHint) statusStripHint.textContent = 'Open Owlmic on PC or plug in USB';
                if (statusTile) {
                    statusTile.classList.remove('is-pulsing');
                    statusTile.classList.add('is-alert');
                }
            } else if (state.connection === 'repair') {
                mobileStatusStrip.style.display = 'flex';
                if (statusStripDevice) statusStripDevice.textContent = 'PC Device Needs Repair';
                if (statusStripCode) statusStripCode.style.display = 'none';
                if (statusStripHint) statusStripHint.textContent = 'Click Repair on PC companion';
                if (statusTile) statusTile.classList.remove('is-pulsing', 'is-alert');
                if (indicatorIcon) indicatorIcon.style.color = '#FFD60A';
            } else {
                // Connected
                mobileStatusStrip.style.display = 'none';
                if (statusTile) statusTile.classList.remove('is-pulsing', 'is-alert');
                if (indicatorIcon) {
                    indicatorIcon.style.color = (statusTile && statusTile.classList.contains('is-on')) ? '#000000' : '#E6E6E6';
                }
            }
        }

        if (waveformDock) {
            const hasStrip = mobileStatusStrip && mobileStatusStrip.style.display !== 'none';
            waveformDock.classList.toggle('has-strip', hasStrip && waveformDock.classList.contains('is-docked'));
        }
    });
});
