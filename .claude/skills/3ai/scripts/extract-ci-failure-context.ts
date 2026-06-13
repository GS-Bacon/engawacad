/**
 * ci.log から失敗周辺の抜粋を返す (#150)。
 *
 * - `error:`, `FAILED`, `panicked at`, `error[E` のいずれかが現れる最後の位置を探す
 * - その行の前後 maxLines/2 行ずつを抜き出して返す
 * - どれもヒットしなければ末尾 maxLines 行を返す
 * - 入力が空 / 全長が maxLines 以下なら全文 (or 空) を返す
 */
export function extractCiFailureContext(ciLog: string, maxLines: number = 40): string {
  if (!ciLog) return "";
  const lines = ciLog.split("\n");
  if (lines.length <= maxLines) return ciLog.trimEnd();

  const KEYWORDS = ["error:", "FAILED", "panicked at", "error[E"];
  let lastHit = -1;
  for (let i = lines.length - 1; i >= 0; i--) {
    if (KEYWORDS.some((k) => lines[i].includes(k))) {
      lastHit = i;
      break;
    }
  }

  if (lastHit < 0) {
    return lines.slice(-maxLines).join("\n").trimEnd();
  }

  const half = Math.floor(maxLines / 2);
  const start = Math.max(0, lastHit - half);
  const end = Math.min(lines.length, lastHit + half + 1);
  return lines.slice(start, end).join("\n").trimEnd();
}
