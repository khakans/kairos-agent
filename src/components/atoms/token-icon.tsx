export function TokenIcon({ symbol }: { symbol: string }) {
  return (
    <span
      aria-hidden="true"
      className={`token-icon token-${symbol.toLowerCase()}`}
    >
      {symbol === "SOL"
        ? "≋"
        : symbol === "JUP"
          ? "◒"
          : symbol === "RAY"
            ? "R"
            : "$"}
    </span>
  );
}
