function installCanvasTouchNavigation({
    canvas,
    listen,
    localPoint,
    modifiers,
    send,
    forwardPointer,
}) {
    const TAP_MOVE_THRESHOLD_SQ = 36;
    const touches = new Map();
    let touchSequence = null;
    let gestureBaseline = null;
    let latestGesture = null;
    let gestureFrame = 0;

    const sendPointerAt = (kind, point, event, detail = 0) => {
        send({
            type: "pointer",
            kind,
            x: point[0],
            y: point[1],
            button: 0,
            detail,
            ...modifiers(event),
        });
    };

    const touchGeometry = () => {
        const points = Array.from(touches.values()).slice(0, 2);
        if (!points.length) return null;
        if (points.length === 1) {
            return { count: 1, center: points[0], distance: 0 };
        }
        return {
            count: 2,
            center: [
                (points[0][0] + points[1][0]) / 2,
                (points[0][1] + points[1][1]) / 2,
            ],
            distance: Math.hypot(
                points[1][0] - points[0][0],
                points[1][1] - points[0][1],
            ),
        };
    };

    const flushTouchTransform = () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        const previous = gestureBaseline;
        const current = latestGesture;
        if (!previous || !current || previous.count !== current.count) {
            gestureBaseline = current;
            return;
        }
        const zoom = current.count === 2 && previous.distance > 0
            ? current.distance / previous.distance
            : 1;
        const dx = current.center[0] - previous.center[0];
        const dy = current.center[1] - previous.center[1];
        if (Math.abs(zoom - 1) > 0.0001 || Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
            send({
                type: "touch_transform",
                zoom,
                dx,
                dy,
                x: current.center[0],
                y: current.center[1],
            });
        }
        gestureBaseline = current;
    };

    const queueTouchTransform = () => {
        latestGesture = touchGeometry();
        if (!gestureFrame) {
            gestureFrame = requestAnimationFrame(() => {
                gestureFrame = 0;
                flushTouchTransform();
            });
        }
    };

    const resetTouchBaseline = () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        latestGesture = touchGeometry();
        gestureBaseline = latestGesture;
    };

    const beginTouch = (event) => {
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        try {
            canvas.setPointerCapture(event.pointerId);
        } catch (_) {
            // Capture can fail if the browser already cancelled this pointer.
        }
        if (touchSequence?.navigating) flushTouchTransform();
        const point = localPoint(event);
        if (!touches.size) {
            touchSequence = {
                pointerId: event.pointerId,
                start: point,
                navigating: false,
                multi: false,
            };
        }
        touches.set(event.pointerId, point);
        if (touches.size >= 2 && touchSequence) {
            touchSequence.multi = true;
            touchSequence.navigating = true;
        }
        resetTouchBaseline();
    };

    const moveTouch = (event) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        touches.set(event.pointerId, localPoint(event));
        if (touchSequence && !touchSequence.navigating) {
            const primary = touches.get(touchSequence.pointerId);
            if (primary) {
                const dx = primary[0] - touchSequence.start[0];
                const dy = primary[1] - touchSequence.start[1];
                touchSequence.navigating = dx * dx + dy * dy > TAP_MOVE_THRESHOLD_SQ;
            }
        }
        if (touchSequence?.navigating) queueTouchTransform();
    };

    const finishTouch = (event, allowTap) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        const point = localPoint(event);
        touches.set(event.pointerId, point);
        if (touchSequence?.pointerId === event.pointerId && !touchSequence.navigating) {
            const dx = point[0] - touchSequence.start[0];
            const dy = point[1] - touchSequence.start[1];
            touchSequence.navigating = dx * dx + dy * dy > TAP_MOVE_THRESHOLD_SQ;
        }
        if (touchSequence?.navigating) {
            latestGesture = touchGeometry();
            flushTouchTransform();
        }
        const isTap = Boolean(
            allowTap &&
            touches.size === 1 &&
            touchSequence?.pointerId === event.pointerId &&
            !touchSequence.navigating &&
            !touchSequence.multi,
        );
        touches.delete(event.pointerId);
        if (touches.size) {
            if (touchSequence) {
                touchSequence.multi = true;
                touchSequence.navigating = true;
            }
        } else {
            touchSequence = null;
        }
        resetTouchBaseline();
        if (isTap) {
            sendPointerAt("down", point, event, 1);
            sendPointerAt("up", point, event, 1);
        }
    };

    listen(canvas, "pointerenter", (event) => {
        if (event.pointerType !== "touch") forwardPointer("enter", event);
    });
    listen(canvas, "pointerleave", (event) => {
        if (event.pointerType !== "touch") forwardPointer("leave", event);
    });
    listen(canvas, "pointerdown", (event) => {
        if (event.pointerType === "touch") {
            beginTouch(event);
            return;
        }
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        canvas.setPointerCapture(event.pointerId);
        forwardPointer("down", event);
    }, { passive: false });
    listen(canvas, "pointermove", (event) => {
        if (event.pointerType === "touch") {
            moveTouch(event);
            return;
        }
        forwardPointer("move", event, true);
    }, { passive: false });
    listen(canvas, "pointerup", (event) => {
        if (event.pointerType === "touch") finishTouch(event, true);
        else forwardPointer("up", event);
    }, { passive: false });
    listen(canvas, "pointercancel", (event) => {
        if (event.pointerType === "touch") finishTouch(event, false);
        else forwardPointer("cancel", event);
    }, { passive: false });
    listen(canvas, "lostpointercapture", (event) => {
        if (event.pointerType === "touch" && touches.has(event.pointerId)) {
            finishTouch(event, false);
        }
    });

    return () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        touches.clear();
    };
}

if (typeof module !== "undefined") {
    module.exports = { installCanvasTouchNavigation };
}
