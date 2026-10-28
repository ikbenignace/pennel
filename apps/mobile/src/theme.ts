import tokens from '../../../assets/brand/tokens.json' with { type: 'json' };

export type Theme = ReturnType<typeof makeTheme>;
export function makeTheme(mode: 'light' | 'dark') {
  return {
    mode,
    colors: Object.fromEntries(Object.entries(tokens.colors).map(([role, values]) => [role, values[mode]])) as Record<keyof typeof tokens.colors, string>,
    space: tokens.spacing,
    radius: tokens.radii,
    font: { regular: 'Figtree-Regular', medium: 'Figtree-Medium', semibold: 'Figtree-SemiBold', display: 'InstrumentSerif-Regular' },
    size: tokens.typography.sizes,
  };
}
