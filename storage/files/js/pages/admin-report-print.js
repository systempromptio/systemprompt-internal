/**
 * Print action for the month-end reports.
 *
 * The PDF a customer receives is the browser's own print of this page, so the
 * report they read and the report the operator reads are the same rendering.
 * Everything that makes it printable is CSS (24-reports.css); this is only the
 * button.
 */
document.addEventListener('click', (event) => {
    const trigger = event.target.closest('[data-print-report]');
    if (!trigger) {
        return;
    }
    window.print();
});
