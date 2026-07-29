const FEEDBACK_MS = 2000;

export function flashCopied(button) {
  button.classList.add('copied');
  setTimeout(() => button.classList.remove('copied'), FEEDBACK_MS);
}

export function initCopyButtons(root = document) {
  for (const button of root.querySelectorAll('.copy-btn')) {
    if (button.dataset.copyBound === 'true') continue;
    button.dataset.copyBound = 'true';

    button.addEventListener('click', () => {
      const code = button.dataset.code;
      if (!code) return;
      navigator.clipboard.writeText(code).then(
        () => flashCopied(button),
        () => button.classList.add('copy-failed')
      );
    });
  }
}
