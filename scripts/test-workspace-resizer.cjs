"use strict";

const assert = require("node:assert/strict");

function makeClassList(...initial) {
    const values = new Set(initial);
    return {
        add: (...classes) => classes.forEach((value) => values.add(value)),
        remove: (...classes) => classes.forEach((value) => values.delete(value)),
        contains: (value) => values.has(value),
    };
}

const documentListeners = new Map();
const windowListeners = new Map();
const stored = new Map();
const frames = new Map();
const widths = { left: 300, right: 380 };
const defaults = { ...widths };
let nextFrame = 1;

const panels = {
    left: { getBoundingClientRect: () => ({ width: widths.left }) },
    right: { getBoundingClientRect: () => ({ width: widths.right }) },
};
const handles = {};
const shell = {
    classList: makeClassList("rton-workspace-shell"),
    style: {
        setProperty(name, value) {
            widths[name.includes("left") ? "left" : "right"] = Number.parseInt(value, 10);
        },
        removeProperty(name) {
            const side = name.includes("left") ? "left" : "right";
            widths[side] = defaults[side];
        },
    },
    querySelector(selector) {
        return selector.includes("left") ? panels.left : panels.right;
    },
    querySelectorAll(selector) {
        return selector === ".rton-resize-handle" ? [handles.left, handles.right] : [];
    },
};

function makeHandle(side) {
    const attributes = new Map();
    return {
        attributes,
        classList: makeClassList("rton-resize-handle", `rton-resize-handle-${side}`),
        closest(selector) {
            if (selector === ".rton-resize-handle") return this;
            if (selector === ".rton-workspace-shell") return shell;
            return null;
        },
        setAttribute(name, value) {
            attributes.set(name, value);
        },
        setPointerCapture() {},
        releasePointerCapture() {},
    };
}

handles.left = makeHandle("left");
handles.right = makeHandle("right");

global.localStorage = {
    getItem: (key) => stored.get(key) ?? null,
    setItem: (key, value) => stored.set(key, value),
    removeItem: (key) => stored.delete(key),
};
global.matchMedia = () => ({ matches: true });
global.requestAnimationFrame = (callback) => {
    const id = nextFrame++;
    frames.set(id, callback);
    return id;
};
global.cancelAnimationFrame = (id) => frames.delete(id);
global.document = {
    documentElement: { classList: makeClassList() },
    addEventListener: (name, callback) => documentListeners.set(name, callback),
    removeEventListener: (name) => documentListeners.delete(name),
    querySelectorAll: (selector) => selector === ".rton-workspace-shell" ? [shell] : [],
};
global.window = {
    addEventListener: (name, callback) => windowListeners.set(name, callback),
    removeEventListener: (name) => windowListeners.delete(name),
};

require("../app/assets/workspace_resizer.js");

function flushFrames() {
    const pending = Array.from(frames.values());
    frames.clear();
    pending.forEach((callback) => callback(0));
}

function pointerEvent(target, pointerId, clientX) {
    return {
        target,
        pointerId,
        clientX,
        button: 0,
        shiftKey: false,
        preventDefault() {},
    };
}

flushFrames();
assert.equal(handles.left.attributes.get("aria-valuemin"), "220");
assert.equal(handles.left.attributes.get("aria-valuemax"), "560");
assert.equal(handles.right.attributes.get("aria-valuenow"), "380");

documentListeners.get("pointerdown")(pointerEvent(handles.left, 1, 300));
documentListeners.get("pointermove")(pointerEvent(handles.left, 1, 340));
documentListeners.get("pointermove")(pointerEvent(handles.left, 1, 380));
assert.equal(frames.size, 1, "pointer moves should be coalesced into one animation frame");
flushFrames();
assert.equal(widths.left, 380);
documentListeners.get("pointerup")(pointerEvent(handles.left, 1, 380));
assert.equal(handles.left.attributes.get("aria-valuenow"), "380");
assert.equal(stored.get("badpiggies-editor-left-panel-width"), "380");

documentListeners.get("pointerdown")(pointerEvent(handles.right, 2, 1000));
documentListeners.get("pointermove")(pointerEvent(handles.right, 2, 900));
flushFrames();
documentListeners.get("pointerup")(pointerEvent(handles.right, 2, 900));
assert.equal(widths.right, 480, "dragging the right handle left should widen the panel");

documentListeners.get("pointerdown")(pointerEvent(handles.left, 3, 380));
documentListeners.get("pointermove")(pointerEvent(handles.left, 3, 2000));
flushFrames();
documentListeners.get("pointerup")(pointerEvent(handles.left, 3, 2000));
assert.equal(widths.left, 560);

documentListeners.get("keydown")({
    ...pointerEvent(handles.right, 4, 0),
    key: "Home",
});
assert.equal(widths.right, 220);
documentListeners.get("keydown")({
    ...pointerEvent(handles.right, 4, 0),
    key: "End",
});
assert.equal(widths.right, 560);

documentListeners.get("dblclick")({ target: handles.left });
assert.equal(widths.left, 300);
assert.equal(stored.has("badpiggies-editor-left-panel-width"), false);

window.bpWorkspacePanels.dispose();
console.log("workspace resizer tests passed");
