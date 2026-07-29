import { flashCopied } from './copy-buttons.js';

function buildMarkdown() {
  const title = document.querySelector('.docs-header h1');
  const description = document.querySelector('.docs-description');
  const content = document.querySelector('.docs-content');

  const parts = [];
  if (title) parts.push(`# ${title.textContent}\n`);
  if (description) parts.push(`${description.textContent}\n`);
  if (content) parts.push(content.innerText);

  return parts.join('\n');
}

export function initExportMarkdown() {
  const button = document.querySelector('.docs-export-btn');
  if (!button) return;

  button.addEventListener('click', () => {
    navigator.clipboard.writeText(buildMarkdown()).then(
      () => flashCopied(button),
      () => button.classList.add('copy-failed')
    );
  });
}
