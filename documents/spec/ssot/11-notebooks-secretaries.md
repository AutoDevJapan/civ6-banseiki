## 11. Notebooks、Next Action、Secretaries

### 11.1 UI配置

既存メニューのList of Reports左側にNotebooksを置く。

```text
[ World Rankings ] [ Era Progress ] [ Notebooks ] [ List of Reports ]
```

```text
Notebooks
├─ Journals
│  ├─ Character Journal
│  ├─ Law Journal
│  └─ Religion Journal
│
└─ National Treasury
   ├─ Secretaries
   └─ Vault
```

- `Notebooks`は長期参照・設定を集める国家台帳
- 中身がまだ無い項目も表示し、項目名に仮の表記「（未実装）／(not yet)／(не готово)」を付ける。表記は用語辞書（14-3）に収録する
- `List of Reports`は現在起きている報告・ログ
- 既存通知は即時対応が必要な出来事
- 襲来・遠征のLogは既存通知とList of Reportsに統合する

### 11.2 Next Action

重要な未対応案件は、Unit操作と同様にNext Actionとして表示する。

- 対応する
- スキップ
- 次ターンまで保留
- 自動判断
- 方針を設定

| 案件 | Next Action例 |
|---|---|
| 飢餓地域 | 食料輸送、備蓄放出、避難、放置 |
| 災害都市 | 人員派遣、建材輸送、復興、避難 |
| 異宇宙襲来 | 偵察、迎撃、封鎖、交渉、撤退 |
| 暗号通信 | 解読開始、研究都市変更、AGI投入 |
| 世界法律 | 提案、投票、保留 |
| 宗教再興 | 保護、再興、研究、抑圧、放置 |
| 神の予兆 | 供物、防護、避難、受諾 |
| 遠征報告 | 支援、撤退、方針変更、救出 |

### 11.3 Secretaries

Secretariesは開始時に存在しない。Great Personを獲得して任命し、反復処理を委任できる進行報酬である。

| Secretary | 主な領域 |
|---|---|
| 開発長官 | 科学、技術、都市開発、暗号解読、研究、宇宙開拓 |
| 国務長官 | 外交、世界法律、宗教、同盟、自治、吸収、遠征外交 |
| 国防長官 | 軍事、防衛、治療、襲来、反乱、都市防衛、危機対応 |
| 総務長官 | 経済、物流、内政、人事、装備、Artifact、備蓄、都市運営 |

- Secretaryはプレイヤーの方針・禁止事項・予算の範囲で反復処理を行う
- 長官なし、または委任しない場合は完全手動操作できる
- 委任レベル: 厳格／標準／裁量／独断
- 不可逆行為、宣戦、併合、宗教抑圧、重要人物処遇、Artifact消費、重要法律変更は原則プレイヤー承認
- 任命したGreat Personの性格・特性・能力により自動化の傾向・精度・裁量が変わる

---
