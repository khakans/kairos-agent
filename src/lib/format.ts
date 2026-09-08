export const usd = (value: string | number, digits = 2) =>
  new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(Number(value));
export const number = (value: string | number, digits = 4) =>
  new Intl.NumberFormat("en-US", { maximumFractionDigits: digits }).format(
    Number(value),
  );
export const time = (timestamp: number) =>
  new Date(timestamp * 1000).toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
export const short = (value: string) =>
  `${value.slice(0, 6)}…${value.slice(-4)}`;
