(() => {
    const element = node => {
        switch (node?.tagName) {
            case 'INPUT': return 'input';
            case 'TEXTAREA': return 'textarea';
            case 'BODY': return 'body';
            default: return 'other';
        }
    };
    const report = (event, target) => window.ipc.postMessage(JSON.stringify({
        event,
        target: element(target),
        active: element(document.activeElement),
        document_focused: document.hasFocus(),
        top_frame: window === window.top,
    }));
    for (const name of ['focusin', 'focusout', 'pointerdown', 'keydown', 'input']) {
        document.addEventListener(name, event => report(name, event.target), true);
    }
    for (const name of ['focus', 'blur']) {
        window.addEventListener(name, () => report(name, document.activeElement));
    }
    document.addEventListener('DOMContentLoaded', () => report('ready', document.activeElement), {once: true});
})();
