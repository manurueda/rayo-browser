# Design: Fix Wait and Clear

## MutationObserver null-safe target
Replace:
```javascript
observer.observe(document.body, { childList: true, subtree: true });
```
With:
```javascript
const target = document.body || document.documentElement;
observer.observe(target, { childList: true, subtree: true });
```

## Replace execCommand with value clearing
Replace the select() + execCommand('delete') approach with:
```javascript
const el = document.querySelector(sel);
el.value = '';
el.dispatchEvent(new Event('input', { bubbles: true }));
el.dispatchEvent(new Event('change', { bubbles: true }));
```
This is simpler, non-deprecated, and works for all input/textarea elements.
