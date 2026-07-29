export function throttle(fn, limit) {
  let waiting = false;
  return (...args) => {
    if (waiting) return;
    waiting = true;
    setTimeout(() => {
      waiting = false;
    }, limit);
    fn(...args);
  };
}
