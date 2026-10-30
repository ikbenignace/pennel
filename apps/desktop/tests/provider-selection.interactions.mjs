export async function checkProviderSelection() {
  if (!window.__PENNEL_FIXTURE__) throw new Error('Synthetic fixture required');
  const until = async (check) => {
    for (let i = 0; i < 100; i++) {
      if (check()) return;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    throw new Error('Selected provider did not open immediately');
  };
  const rows = () => [...document.querySelectorAll('.providers .list > button[aria-current], .providers .list > button:not(.provider-add)')];
  await until(() => rows().length >= 2);
  const targetName = rows()[1].querySelector('.provider-name').textContent.trim();
  rows()[1].click();
  await until(() => {
    const active = document.querySelector('.providers .list > button[aria-current="true"] .provider-name');
    const detail = document.querySelector('.providers input[aria-label]');
    return active?.textContent.trim() === targetName && detail?.value === targetName;
  });
  const name = document.querySelector('.providers input[aria-label]');
  name.value = 'Synthetic renamed provider';
  name.dispatchEvent(new Event('change', { bubbles: true }));
  rows()[0].click();
  await until(() => document.querySelector('.providers .list > button[aria-current="true"] .provider-name')?.textContent.trim() !== targetName);
  await until(() => rows().some(row => row.querySelector('.provider-name')?.textContent.trim() === 'Synthetic renamed provider'));
  rows().find(row => row.querySelector('.provider-name')?.textContent.trim() === 'Synthetic renamed provider').click();
  await until(() => document.querySelector('.providers input[aria-label]')?.value === 'Synthetic renamed provider');
  return { selected: targetName, editPreserved: true };
}
