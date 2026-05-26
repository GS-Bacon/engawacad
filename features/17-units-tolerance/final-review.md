issues:
  - id: F01
    severity: high
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 70
    finding: "既知の `push_triangle` の誤りを『別 Issue として切り出す』と deferral しているが、後続 issue 番号・リンクが記載されておらず、先送り事項の追跡先が存在しない。"
    suggestion: "この既知不具合を受ける実在 issue を起票して番号を明記するか、既存 issue に委譲するならその issue 番号を ADR に追記すること。"
  - id: F02
    severity: medium
    file: "docs/decisions/003-viewer-and-app-architecture.md"
    line_hint: 88
    finding: "単位系・グローバル公差の委譲先が `Issue I-4` のままで、同テーマを実際に扱っている `Issue #17` と識別子が食い違っている。関連 ADR/issue の相互参照が文書間で一貫していない。"
    suggestion: "ADR-003 の委譲先を実在する issue 番号に更新し、必要なら ADR-004 側にも関連 issue を明記して双方向のトレーサビリティを揃えること。"

verdict: fail