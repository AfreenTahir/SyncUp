export function BrandLogo({ compact = false }: { compact?: boolean }) {
  return <div className={`sync-brand ${compact ? "compact" : ""}`}>
    <svg className="sync-logo" viewBox="0 0 64 64" role="img" aria-label="SyncUp">
      <path d="M18 20 32 12l15 10M18 20l1 22 15 10m13-30-1 20-12 10M18 20l28 22M47 22 19 42"/>
      <circle cx="18" cy="20" r="6"/><circle cx="32" cy="12" r="5"/><circle cx="47" cy="22" r="7"/><circle cx="46" cy="42" r="5"/><circle cx="34" cy="52" r="6"/><circle cx="19" cy="42" r="5"/>
      <path className="logo-spark" d="m32 25 2.2 5.2L40 32l-5.8 1.8L32 39l-2.2-5.2L24 32l5.8-1.8Z"/>
    </svg>
    <span><strong>SyncUp</strong>{!compact && <small>Play better together</small>}</span>
  </div>;
}

export function VisualOption({ id, label = "Choice" }: { id?: string; label?: string }) {
  if (!id) return null;
  if (id === "generated") {
    const icons = ["✦","☀","☾","♥","♬","☕","⌂","⚡","✈","♛","☺","◆"];
    const hash = [...label].reduce((sum, character) => sum + character.charCodeAt(0), 0);
    return <span className={`visual-option generated-visual visual-tone-${hash % 6}`} role="img" aria-label={`${label} illustration`}><b aria-hidden="true">{icons[hash % icons.length]}</b><em>{label}</em></span>;
  }
  const [sheet, rawIndex] = id.split(":");
  const index = Number(rawIndex);
  return <span className={`visual-option sheet-${sheet} sprite-${index}`} role="img" aria-label={`${label} illustration`}/>;
}
