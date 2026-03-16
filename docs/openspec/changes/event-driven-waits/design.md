# Design: Event-Driven Waits

## Implementation
Single CDP evaluate call with a Promise that uses MutationObserver:

```javascript
new Promise((resolve, reject) => {
    const el = document.querySelector(selector);
    if (el) { resolve(true); return; }
    const observer = new MutationObserver(() => {
        if (document.querySelector(selector)) {
            observer.disconnect();
            resolve(true);
        }
    });
    observer.observe(document.body, { childList: true, subtree: true });
    setTimeout(() => { observer.disconnect(); reject(new Error('timeout')); }, timeoutMs);
})
```

## Why this works
- `page.evaluate()` with a Promise awaits its resolution via CDP Runtime.awaitPromise
- MutationObserver fires synchronously on DOM mutation, then the querySelector check runs
- No polling round-trips — the entire wait happens browser-side
- Timeout cleanup via setTimeout ensures no leaked observers
