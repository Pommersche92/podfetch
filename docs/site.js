document.addEventListener('DOMContentLoaded', () => {
  const elements = document.querySelectorAll('.fade-in-up');
  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add('visible');
      }
    });
  }, { threshold: 0.15 });

  elements.forEach((el) => observer.observe(el));

  const banner = document.getElementById('dsgvo-banner');
  const acceptButton = document.getElementById('dsgvo-accept');

  if (!banner || !acceptButton) return;

  if (!localStorage.getItem('podfetch-dsgvo-accepted')) {
    banner.hidden = false;
  }

  acceptButton.addEventListener('click', () => {
    localStorage.setItem('podfetch-dsgvo-accepted', '1');
    banner.hidden = true;
  });
});
