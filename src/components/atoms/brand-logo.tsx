/** Original Kairos artwork; the viewport removes transparent margins without altering the source. */
export function BrandLogo({ mark = false }: { mark?: boolean }) {
  return (
    <svg
      className={`brand-logo${mark ? " brand-logo--mark" : ""}`}
      viewBox={mark ? "245 167 819 927" : "205 320 1207 371"}
      role="img"
      aria-label="Kairos"
      focusable="false"
    >
      <image
        href={mark ? "/brand/kairos-mark.png" : "/brand/kairos-wordmark.png"}
        width={mark ? 1254 : 1586}
        height={mark ? 1254 : 992}
      />
    </svg>
  );
}
