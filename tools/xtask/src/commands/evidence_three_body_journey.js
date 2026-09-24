for (const button of document.querySelectorAll('[data-view]')) {
  button.addEventListener('click', () => {
    const view = button.dataset.view;
    for (const option of document.querySelectorAll('[data-view]')) option.setAttribute('aria-pressed', String(option === button));
    for (const panel of document.querySelectorAll('[data-body]')) panel.hidden = view !== 'all' && panel.dataset.body !== view;
    for (const comparison of document.querySelectorAll('.comparison')) comparison.classList.toggle('single', view !== 'all');
  });
}
for (const transcript of document.querySelectorAll('[data-transcript]')) {
  fetch(transcript.dataset.transcript).then(response => {
    if (!response.ok) throw new Error('Transcript unavailable');
    return response.text();
  }).then(text => { transcript.textContent = text; }).catch(() => {});
}
