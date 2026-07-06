// zai-model.ts — GLM (Z.AI) モデル識別子の単一の真実源 (#309)
//
// 背景: dispatch-glm.ts / glm-via-zai.ts / dispatch-glm-review.ts の 3 本で
// Z.AI に渡すモデル識別子が "GLM-5.1" / "glm-4.6" / "claude-opus-4-5-20251101" と
// 混在していた。いずれも Z.AI の Anthropic 互換エンドポイントに
// ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL として渡る同じ値なので、
// 定数を 1 箇所に集約して再発を防ぐ。

/** Z.AI に渡す既定の GLM モデル識別子。GLM_MODEL env が無いときの fallback。 */
export const DEFAULT_GLM_MODEL = "GLM-5.1";

/** GLM_MODEL env があればそれを優先し、無ければ DEFAULT_GLM_MODEL を返す。
 *  precedence は既存の dispatch-glm-review.ts の `process.env.GLM_MODEL ?? ...` を踏襲
 *  (env 未設定 = null/undefined のときだけ default に落とす)。 */
export function resolveGlmModel(): string {
  return process.env.GLM_MODEL ?? DEFAULT_GLM_MODEL;
}

/** Z.AI 経由の claude -p に渡す ANTHROPIC_DEFAULT_*_MODEL 3 キーを構築する。
 *  3 スクリプトで重複していた 3 行を集約する。model 省略時は resolveGlmModel() で解決。 */
export function buildAnthropicModelEnv(model: string = resolveGlmModel()): {
  ANTHROPIC_DEFAULT_OPUS_MODEL: string;
  ANTHROPIC_DEFAULT_SONNET_MODEL: string;
  ANTHROPIC_DEFAULT_HAIKU_MODEL: string;
} {
  return {
    ANTHROPIC_DEFAULT_OPUS_MODEL: model,
    ANTHROPIC_DEFAULT_SONNET_MODEL: model,
    ANTHROPIC_DEFAULT_HAIKU_MODEL: model,
  };
}
