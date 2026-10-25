// Mirrors the public metadata projection in core/catalog.rs and core/models.rs.
export function visualModels(catalog, source, language = "nl") {
  const block = source.match(/pub const RECOMMENDED:[\s\S]*?=\s*\[([\s\S]*?)\];/);
  if (!block) throw new Error("Could not read the core's recommended model order");
  const recommended = [...block[1].matchAll(/"([^"]+)"/g)].map(match => match[1]);
  const round = number => Math.floor(number + 0.5);
  const models = catalog.models.filter(model => model.role === "asr" && model.files[model.quant]).map(model => {
    const results = catalog.results?.[model.id];
    const rtf = results?.performance?.accelerated ?? results?.performance?.cpu;
    const error = results?.accuracy?.[language]?.error;
    return {
      id: model.id, name: model.name,
      description: model.description || (model.languages.length === 1 ? `Spraak naar tekst, alleen ${model.languages[0]}` : `Spraak naar tekst in ${model.languages.length} talen`),
      size_bytes: model.files[model.quant].size, parameters: model.parameters ?? null,
      speed: rtf > 0 ? round(100 * (1 - Math.exp(-rtf / 8))) : 10,
      accuracy: error === undefined ? null : round(100 * Math.exp(-error / 15)),
      languages: model.languages, recommended: recommended.includes(model.id), legacy: false,
    };
  });
  const rank = model => { const index = recommended.indexOf(model.id); return index < 0 ? Number.MAX_SAFE_INTEGER : index; };
  return models.sort((a, b) => rank(a) - rank(b) || Number(b.accuracy !== null) - Number(a.accuracy !== null) || (b.accuracy ?? 0) - (a.accuracy ?? 0) || b.speed - a.speed);
}

export function visualCallApps(source) {
  return ["CALL_APPS", "BROWSERS"].flatMap(name => {
    const block = source.match(new RegExp(`const ${name}:[\\s\\S]*?=\\s*&\\[([\\s\\S]*?)\\];`));
    if (!block) throw new Error(`Could not read ${name} from core rules`);
    return [...block[1].matchAll(/"([^"]+)"/g)].map(match => match[1]);
  });
}
