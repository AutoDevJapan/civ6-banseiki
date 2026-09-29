"""The 1534 scenario's IDs and translated leader names.

Territory/castle evidence lives in documents/spec/data/japan-1534-start.json.
The two deliberate head overrides follow SSOT 3.1.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "documents/spec/data/japan-1534-start.json"

# (ID suffix, English display name, Russian display name). Oda/Takeda keep
# their existing IDs and artwork. The other 20 use the Japan-symbol placeholder.
HEADS = {
    "ODA": ("NOBUHIDE", "Oda Nobuhide", "Ода Нобухидэ"),
    "DATE": ("TANEMUNE", "Date Tanemune", "Датэ Танэмунэ"),
    "ASHINA": ("MORIKIYO", "Ashina Morikiyo", "Асина Морикиё"),
    "HOJO": ("UJITSUNA", "Hojo Ujitsuna", "Ходзё Удзицуна"),
    "YAMANOUCHI_UESUGI": ("NORIMASA", "Uesugi Norimasa", "Уэсуги Норимаса"),
    "TAKEDA": ("NOBUTORA", "Takeda Nobutora", "Такэда Нобутора"),
    "NAGAO": ("TAMEKAGE", "Nagao Tamekage", "Нагао Тамэкагэ"),
    "IMAGAWA": ("UJITERU", "Imagawa Ujiteru", "Имагава Удзитэру"),
    "MATSUDAIRA": ("KIYOYASU", "Matsudaira Kiyoyasu", "Мацудайра Киёясу"),
    "ASAKURA": ("TAKAKAGE", "Asakura Takakage", "Асакура Такакагэ"),
    "ASHIKAGA": ("YOSHIHARU", "Ashikaga Yoshiharu", "Асикага Ёсихару"),
    "MIYOSHI": ("NAGAYOSHI", "Miyoshi Nagayoshi", "Миёси Нагаёси"),
    "ROKKAKU": ("SADAYORI", "Rokkaku Sadayori", "Роккаку Садаёри"),
    "HONGANJI": ("SHONYO", "Shonyo", "Сёнё"),
    "OUCHI": ("YOSHITAKA", "Ouchi Yoshitaka", "Оути Ёситака"),
    "AMAGO": ("TSUNEHISA", "Amago Tsunehisa", "Амаго Цунэхиса"),
    "MORI": ("MOTONARI", "Mori Motonari", "Мори Мотонари"),
    "CHOSOKABE": ("KUNICHIKA", "Chosokabe Kunichika", "Тёсокабэ Кунитика"),
    "OTOMO": ("YOSHIAKI", "Otomo Yoshiaki", "Отомо Ёсиаки"),
    "SHIMAZU": ("TAKAHISA", "Shimazu Takahisa", "Симадзу Такахиса"),
    "RYUKYU": ("SHO_SEI", "King Sho Sei", "Король Сё Сэй"),
    "AINU": ("TARIKONA", "Tarikona", "Тарикона"),
}
HEAD_OVERRIDES = {"SHIMAZU": "島津貴久", "AINU": "タリコナ"}


def data() -> dict:
    return json.loads(DATA.read_text(encoding="utf-8"))


def leader_id(slug: str) -> str:
    return f"LEADER_JAC_{slug}_{HEADS[slug][0]}"


def civ_id(slug: str) -> str:
    return f"CIVILIZATION_JAC_{slug}"
