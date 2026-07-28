import { throttle } from './dom-throttle.js';

const SCROLL_OFFSET = 100;

function collectHeadings(links) {
  const headings = [];
  for (const link of links) {
    const href = link.getAttribute('href');
    if (!href || !href.startsWith('#')) continue;
    const heading = document.getElementById(href.slice(1));
    if (heading) headings.push({ link, heading });
  }
  return headings;
}

export function initTocHighlight() {
  const tocLinks = document.querySelectorAll('.toc-content a, .toc-content--mobile a');
  if (!tocLinks.length) return;

  const headings = collectHeadings(tocLinks);
  if (!headings.length) return;

  const updateActiveLink = () => {
    const scrollTop = window.scrollY;
    let activeIndex = 0;

    headings.forEach((item, index) => {
      if (item.heading.offsetTop - SCROLL_OFFSET <= scrollTop) activeIndex = index;
    });

    for (const link of tocLinks) link.classList.remove('active');
    headings[activeIndex].link.classList.add('active');
  };

  window.addEventListener('scroll', throttle(updateActiveLink, 100), { passive: true });
  updateActiveLink();
}

export function initMobileToc() {
  const mobileDetails = document.querySelector('.toc-mobile-details');
  if (!mobileDetails) return;

  for (const link of mobileDetails.querySelectorAll('a')) {
    link.addEventListener('click', () => {
      setTimeout(() => mobileDetails.removeAttribute('open'), 100);
    });
  }
}
