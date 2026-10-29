export async function checkQ5Visibility() {
  if (!window.__PENNEL_FIXTURE__) throw new Error('Synthetic fixture required');
  for (let i = 0; i < 100; i++) {
    const card = [...document.querySelectorAll('.model-card')].find(item => item.querySelector('strong')?.textContent.includes('Whisper large-v3 turbo (q5)'));
    if (card) {
      if (card.closest('.all-models')) throw new Error('Q5 is hidden under all models');
      const { t } = await import('/src/i18n.ts');
      if ((card.textContent.match(new RegExp(t('providers.local.not_measured'), 'g')) || []).length < 2) throw new Error('Q5 must not present another variant’s benchmark as its own');
      return { visible: true, benchmarks: 'unmeasured' };
    }
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  throw new Error('Q5 model card missing');
}
