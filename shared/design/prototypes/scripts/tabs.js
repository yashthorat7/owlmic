// Device Display Toggles, Viewport Centering & Phone Rotation Engine

document.addEventListener('DOMContentLoaded', () => {
    const viewsContainer = document.querySelector('.views-container');
    const viewPhone = document.getElementById('view-phone');
    const viewPc = document.getElementById('view-pc');
    const deviceMobile = document.querySelector('.js-device-mobile');
    const devicePc = document.querySelector('.device-pc');

    // Device Display Toggle buttons in Top Capsule (Mobile & PC)
    const btnTogglePhone = document.querySelector('.js-toggle-phone');
    const btnTogglePc = document.querySelector('.js-toggle-pc');

    let isPhoneActive = true;
    let isPcActive = true;

    // Simulation Controls in Top Capsule
    const btnRotatePhone = document.querySelector('.js-btn-rotate-phone');
    const btnCycleLink = document.querySelector('.js-btn-cycle-link');
    const btnDisturb = document.querySelector('.js-btn-disturb');

    const topLinkIcon = document.querySelector('.js-top-link-icon');
    const topStateIcon = document.querySelector('.js-top-state-icon');

    function updateDeviceVisibility() {
        if (!viewPhone || !viewPc || !viewsContainer) return;

        // Ensure at least one device is always visible
        if (!isPhoneActive && !isPcActive) {
            isPhoneActive = true;
        }

        // Update button active states and hover tooltips
        if (btnTogglePhone) {
            btnTogglePhone.classList.toggle('active', isPhoneActive);
            btnTogglePhone.title = isPhoneActive ? 'Mobile: ON (Click to toggle off)' : 'Mobile: OFF (Click to toggle on)';
        }
        if (btnTogglePc) {
            btnTogglePc.classList.toggle('active', isPcActive);
            btnTogglePc.title = isPcActive ? 'PC: ON (Click to toggle off)' : 'PC: OFF (Click to toggle on)';
        }

        // --- FLIP Animation Engine for Smooth Layout Transition ---
        // 1. FIRST: Record current bounding box of the active devices before DOM changes
        const phoneRectBefore = isPhoneActive && viewPhone.classList.contains('active') 
            ? viewPhone.getBoundingClientRect() : null;
        const pcRectBefore = isPcActive && viewPc.classList.contains('active') 
            ? viewPc.getBoundingClientRect() : null;

        // 2. DOM State Mutate: update classes
        if (isPhoneActive && isPcActive) {
            // Both devices visible: Side-by-Side Dual View
            viewsContainer.classList.add('is-split-mode');
            viewPhone.classList.add('active');
            viewPhone.classList.remove('is-disabled-device');
            viewPc.classList.add('active');
            viewPc.classList.remove('is-disabled-device');
        } else if (isPhoneActive && !isPcActive) {
            // Mobile only: Centered on screen (PC vanishes cleanly)
            viewsContainer.classList.remove('is-split-mode');
            viewPc.classList.remove('active');
            viewPhone.classList.add('active');
            viewPhone.classList.remove('is-disabled-device');
        } else if (!isPhoneActive && isPcActive) {
            // PC only: Centered on screen (Phone vanishes cleanly)
            viewsContainer.classList.remove('is-split-mode');
            viewPhone.classList.remove('active');
            viewPc.classList.add('active');
            viewPc.classList.remove('is-disabled-device');
        }

        // 3. LAST & INVERT & PLAY: Animate the device that stays or appears to its new center/split position
        requestAnimationFrame(() => {
            if (phoneRectBefore && isPhoneActive) {
                const phoneRectAfter = viewPhone.getBoundingClientRect();
                const deltaX = phoneRectBefore.left - phoneRectAfter.left;
                if (Math.abs(deltaX) > 2) {
                    viewPhone.style.transition = 'none';
                    viewPhone.style.transform = `translateX(${deltaX}px)`;
                    // Force reflow
                    void viewPhone.offsetWidth;
                    viewPhone.style.transition = 'transform 0.45s cubic-bezier(0.16, 1, 0.3, 1)';
                    viewPhone.style.transform = 'translateX(0)';
                    setTimeout(() => {
                        viewPhone.style.transition = '';
                        viewPhone.style.transform = '';
                    }, 480);
                }
            }

            if (pcRectBefore && isPcActive) {
                const pcRectAfter = viewPc.getBoundingClientRect();
                const deltaX = pcRectBefore.left - pcRectAfter.left;
                if (Math.abs(deltaX) > 2) {
                    viewPc.style.transition = 'none';
                    viewPc.style.transform = `translateX(${deltaX}px)`;
                    // Force reflow
                    void viewPc.offsetWidth;
                    viewPc.style.transition = 'transform 0.45s cubic-bezier(0.16, 1, 0.3, 1)';
                    viewPc.style.transform = 'translateX(0)';
                    setTimeout(() => {
                        viewPc.style.transition = '';
                        viewPc.style.transform = '';
                    }, 480);
                }
            }
        });
    }

    if (btnTogglePhone) {
        btnTogglePhone.addEventListener('click', () => {
            if (isPhoneActive) {
                // Turn phone off; if PC was already off, keep PC on so screen isn't blank
                isPhoneActive = false;
                if (!isPcActive) isPcActive = true;
            } else {
                // Turn phone on
                isPhoneActive = true;
            }
            updateDeviceVisibility();
        });
    }

    if (btnTogglePc) {
        btnTogglePc.addEventListener('click', () => {
            if (isPcActive) {
                // Turn PC off; if phone was already off, keep phone on so screen isn't blank
                isPcActive = false;
                if (!isPhoneActive) isPhoneActive = true;
            } else {
                // Turn PC on
                isPcActive = true;
            }
            updateDeviceVisibility();
        });
    }

    // Set initial device layout (both visible side-by-side)
    updateDeviceVisibility();

    function rotatePhoneOrientation(forceOrientation) {
        if (!deviceMobile) return;

        if (window._phoneRotationTimeout) {
            clearTimeout(window._phoneRotationTimeout);
            window._phoneRotationTimeout = null;
        }

        const currentlyLandscape = deviceMobile.classList.contains('is-landscape');
        const shouldBeLandscape = forceOrientation !== undefined 
            ? forceOrientation === 'landscape' 
            : !currentlyLandscape;

        if (shouldBeLandscape) {
            // Rotate to Landscape
            const notifShade = document.querySelector('.js-phone-notif-shade');
            if (notifShade) notifShade.classList.remove('is-open');

            deviceMobile.classList.remove('is-portrait-chassis');
            deviceMobile.classList.add('is-landscape-chassis');

            window._phoneRotationTimeout = setTimeout(() => {
                deviceMobile.classList.remove('is-portrait');
                deviceMobile.classList.add('is-landscape');
                window._phoneRotationTimeout = null;
            }, 250);
        } else {
            // Rotate to Portrait
            deviceMobile.classList.remove('is-landscape-chassis');
            deviceMobile.classList.add('is-portrait-chassis');

            window._phoneRotationTimeout = setTimeout(() => {
                deviceMobile.classList.remove('is-landscape');
                deviceMobile.classList.add('is-portrait');
                window._phoneRotationTimeout = null;
            }, 250);
        }
    }

    // Rotate phone button in top capsule
    if (btnRotatePhone) {
        btnRotatePhone.addEventListener('click', () => {
            rotatePhoneOrientation();
        });
    }

    // Link cycling buttons & icons
    const linkDisplayNames = {
        'wifi': 'Wi-Fi',
        'usb_debugging': 'USB (adb)',
        'usb_tethering': 'USB (rndis)',
        'bluetooth': 'Bluetooth'
    };

    const linkIcons = {
        'wifi': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><line x1="12" y1="20" x2="12.01" y2="20"/></svg>`,
        'usb_debugging': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="7" y="2" width="10" height="20" rx="2"/><line x1="12" y1="6" x2="12.01" y2="6"/><line x1="12" y1="18" x2="12.01" y2="18"/><line x1="10" y1="12" x2="14" y2="12"/></svg>`,
        'usb_tethering': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="7" y="2" width="10" height="20" rx="2"/><line x1="12" y1="6" x2="12.01" y2="6"/><line x1="12" y1="18" x2="12.01" y2="18"/><line x1="10" y1="12" x2="14" y2="12"/></svg>`,
        'bluetooth': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6.5 6.5 17.5 17.5 12 23 12 1 17.5 6.5 6.5 17.5"/></svg>`
    };

    const stateIcons = {
        'connected': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3" fill="currentColor"/><path d="M12 2v3m0 14v3M2 12h3m14 0h3"/></svg>`,
        'approving': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 2v3m0 14v3M2 12h3m14 0h3" stroke-dasharray="2 2"/></svg>`,
        'reconnecting': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 2v3m0 14v3M2 12h3m14 0h3" stroke-dasharray="2 2"/></svg>`,
        'not_connected': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="1" y1="1" x2="23" y2="23"/><circle cx="12" cy="12" r="3"/><path d="M12 2v3m0 14v3M2 12h3m14 0h3"/></svg>`,
        'repair': `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`
    };

    if (btnCycleLink) {
        btnCycleLink.addEventListener('click', () => {
            OwlmicStore.cycleLinkKind();
        });
    }

    // Disturbance / State cycling button
    if (btnDisturb) {
        btnDisturb.addEventListener('click', () => {
            OwlmicStore.disturbConnection();
        });
    }

    // Sync Store state with Top Capsule
    OwlmicStore.subscribe((state) => {
        const linkFormatted = linkDisplayNames[state.link] || state.link;
        const linkTitle = `Link: ${linkFormatted} (Click to cycle)`;

        if (btnCycleLink) btnCycleLink.title = linkTitle;

        if (topLinkIcon && linkIcons[state.link]) {
            topLinkIcon.innerHTML = linkIcons[state.link];
        }

        // Connection State
        const connLabels = {
            'connected': 'Connected',
            'approving': 'Approving',
            'reconnecting': 'Reconnecting',
            'not_connected': 'Offline',
            'repair': 'Needs Repair'
        };
        const connLabel = connLabels[state.connection] || state.connection;
        const stateTitle = `State: ${connLabel} (Click to cycle)`;

        if (btnDisturb) btnDisturb.title = stateTitle;

        const currentSvg = stateIcons[state.connection] || stateIcons['connected'];
        if (topStateIcon) topStateIcon.innerHTML = currentSvg;

        // Hardware Chassis Power State
        if (deviceMobile) {
            deviceMobile.classList.toggle('is-powered-off', !state.appPowered);
        }
        if (devicePc) {
            devicePc.classList.toggle('is-powered-off', !state.pcPowered);
        }
    });

    // --- Draggable Toolbar Engine ---
    const controlsContainer = document.querySelector('.controls-container');
    const dragHandle = document.querySelector('.js-toolbar-drag-handle');

    if (controlsContainer && dragHandle) {
        let isDragging = false;
        let startPointerX = 0;
        let startPointerY = 0;
        let startElemX = 0;
        let startElemY = 0;

        dragHandle.addEventListener('pointerdown', (e) => {
            // Only left click / primary touch
            if (e.button !== 0) return;
            e.preventDefault();
            e.stopPropagation();

            isDragging = true;
            controlsContainer.classList.add('is-dragging');

            const rect = controlsContainer.getBoundingClientRect();
            startElemX = rect.left;
            startElemY = rect.top;
            startPointerX = e.clientX;
            startPointerY = e.clientY;

            // Remove center transform and lock to explicit top & left coordinates
            controlsContainer.style.transform = 'none';
            controlsContainer.style.left = `${startElemX}px`;
            controlsContainer.style.top = `${startElemY}px`;
            controlsContainer.style.bottom = 'auto';
            controlsContainer.style.right = 'auto';

            dragHandle.setPointerCapture(e.pointerId);

            function onPointerMove(moveEvent) {
                if (!isDragging) return;
                moveEvent.preventDefault();

                const deltaX = moveEvent.clientX - startPointerX;
                const deltaY = moveEvent.clientY - startPointerY;

                let nextX = startElemX + deltaX;
                let nextY = startElemY + deltaY;

                // Boundary clamping to viewport edges
                const elemWidth = controlsContainer.offsetWidth;
                const elemHeight = controlsContainer.offsetHeight;
                const minX = 8;
                const maxX = Math.max(minX, window.innerWidth - elemWidth - 8);
                const minY = 8;
                const maxY = Math.max(minY, window.innerHeight - elemHeight - 8);

                nextX = Math.min(Math.max(minX, nextX), maxX);
                nextY = Math.min(Math.max(minY, nextY), maxY);

                controlsContainer.style.left = `${nextX}px`;
                controlsContainer.style.top = `${nextY}px`;
            }

            function onPointerUp(upEvent) {
                if (!isDragging) return;
                isDragging = false;
                controlsContainer.classList.remove('is-dragging');

                try {
                    dragHandle.releasePointerCapture(upEvent.pointerId);
                } catch (_) {}

                document.removeEventListener('pointermove', onPointerMove);
                document.removeEventListener('pointerup', onPointerUp);
                document.removeEventListener('pointercancel', onPointerUp);
            }

            document.addEventListener('pointermove', onPointerMove);
            document.addEventListener('pointerup', onPointerUp);
            document.addEventListener('pointercancel', onPointerUp);
        });
    }

    window.OwlmicNavigation = { 
        rotatePhoneOrientation,
        setDeviceVisibility: (phone, pc) => {
            isPhoneActive = phone;
            isPcActive = pc;
            updateDeviceVisibility();
        }
    };
});
