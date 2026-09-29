"""Generate 1534 leader source art with Codex CLI's built-in image tool.

Only the official Codex CLI with ChatGPT OAuth is used. No API-key image
endpoint is called. Originals and the generation log stay outside Git.

Usage: python tools/jac_art/generate_portraits.py [--only DATE_TANEMUNE ...]
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from PIL import Image

from build_art import ORIGINALS


# Age uses 1534 minus the birth year in documents/spec/data/japan-1534-start.json.
# Where the year is disputed, the prompt and documentation mark it as an estimate.
# The generated faces are reconstructions, never claims of preserved likeness.
SUBJECTS = {
    "ODA_NOBUHIDE": ("織田信秀", "about 23", "young Owari daimyo's dark silk kosode, hakama and eboshi", "an Owari timber castle residence with Muromachi paper screens"),
    "TAKEDA_NOBUTORA": ("武田信虎", "about 40, not the elderly appearance in his 1574 Daisenji portrait", "Kai daimyo's dark red and indigo silk kosode and eboshi", "a Kai daimyo residence with timber, tatami and mountain light"),
    "DATE_TANEMUNE": ("伊達稙宗", "about 46", "formal muted indigo silk kosode and eboshi", "an Oshu timber residence with distant northern hills"),
    "ASHINA_MORIKIYO": ("蘆名盛舜", "about 44", "deep green silk kosode, restrained warrior noble dress", "an Aizu Kurokawa residence with forested mountains"),
    "HOJO_UJITSUNA": ("北条氏綱", "about 47", "dark blue formal kosode and eboshi", "an Odawara reception room with a glimpse of the Sagami coast"),
    "YAMANOUCHI_UESUGI_NORIMASA": ("上杉憲政", "a boy around 11 if the disputed 1523 birth is accepted", "age-appropriate noble boy's Muromachi robes", "a Hirai residence in Kozuke, restrained furnishings"),
    "NAGAO_TAMEKAGE": ("長尾為景", "a mature older adult; birth year uncertain", "weathered indigo silk kosode and eboshi", "a Kasugayama timber residence with Echigo mountain light"),
    "IMAGAWA_UJITERU": ("今川氏輝", "about 21", "refined Suruga court-influenced silk robes", "an Imagawa residence at Sunpu, elegant painted screens"),
    "MATSUDAIRA_KIYOYASU": ("松平清康", "about 23", "practical Mikawa warrior noble silk kosode and hakama", "an Okazaki timber reception room"),
    "ASAKURA_TAKAKAGE": ("朝倉孝景", "about 41; the 1493-born daimyo, not his 15th-century namesake", "cultured Echizen noble silk kosode", "an Ichijodani residence with refined Muromachi screens"),
    "ASHIKAGA_YOSHIHARU": ("足利義晴", "about 23", "Muromachi shogun's formal court robes and eboshi", "a modest shogunal reception room with Kyoto-style screens"),
    "MIYOSHI_NAGAYOSHI": ("三好長慶", "a boy about 12", "age-appropriate Awa noble boy's kosode", "an Awa timber residence, restrained courtly furnishings"),
    "ROKKAKU_SADAYORI": ("六角定頼", "about 39", "formal Omi daimyo's silk kosode and eboshi", "a Kannonji hilltop residence overlooking Omi"),
    "HONGANJI_SHONYO": ("証如", "about 18", "historically plausible Jodo Shinshu head priest's dark robes, shaved head", "an early Ishiyama temple reception hall, wooden beams and tatami"),
    "OUCHI_YOSHITAKA": ("大内義隆", "about 27", "refined Yamaguchi court-influenced silk robes", "a cultured Ouchi residence in Yamaguchi with painted screens"),
    "AMAGO_TSUNEHISA": ("尼子経久", "about 76", "elderly Izumo daimyo's restrained silk kosode and eboshi", "a Gassan Toda hill fortress reception room"),
    "MORI_MOTONARI": ("毛利元就", "about 37", "modest Aki warrior noble's indigo kosode", "a Yoshida Koriyama timber residence amid Aki hills"),
    "CHOSOKABE_KUNICHIKA": ("長宗我部国親", "about 30", "practical Tosa daimyo's dark silk kosode", "an Oko timber residence with Tosa mountain light"),
    "OTOMO_YOSHIAKI": ("大友義鑑", "about 32", "Bungo noble's formal dark teal silk kosode", "a Funai residence with a glimpse of Bungo coastal light"),
    "SHIMAZU_TAKAHISA": ("島津貴久", "about 20; use Takahisa as the game's chosen representative", "young Satsuma noble's formal kosode and hakama", "an early Satsuma timber residence, without later Kagoshima Castle"),
    "RYUKYU_SHO_SEI": ("尚清王", "about 37", "16th-century Ryukyuan royal robes and court headwear, distinct from Japanese samurai attire", "an unoccupied Shuri royal audience room with Ryukyuan architecture"),
    "AINU_TARIKONA": ("タリコナ", "mature adult; birth year and precise appearance unknown", "plausible 16th-century Ainu woven attus robe and simple headband; no later tourist costume", "an unoccupied timber and thatch Ainu communal interior near the Saru River"),
}


def filename(key: str, kind: str) -> str:
    return f"{key.lower()}_{kind}_codex.png"


def prompt(key: str) -> str:
    name, age, clothing, scene = SUBJECTS[key]
    names = {kind: filename(key, kind) for kind in ("portrait", "figure", "background")}
    return f"""Use the built-in image generation tool in this official Codex CLI session to generate three distinct original images of {name} ({key}), the chosen 1534 leader. Generate each asset one at a time and copy its image_gen output into the current directory under the exact requested filename. No API keys, browser, other services, subagents, or changes to other files.

Historical reconstruction: {age}; {clothing}. Use the age and role in 1534, avoiding later-life depictions and anachronistic armor. No surviving image is assumed to establish an exact facial likeness; create a unique credible face. Keep facial features, clothes and palette consistent between portrait and figure. Painterly dimensional strategy-game style, warm directional light and textured brushwork, reminiscent of the visual finish of Civilization VI without copying official figures or pixels. No text, logos or watermarks.

1. {names['portrait']}: square head-and-shoulders portrait, face centered, whole head with generous margins for circular icon crop, neutral warm background, minimum 1024px each side.
2. {names['figure']}: tall near-full-length figure, hands and feet visible, entire silhouette with margin, uniformly vivid magenta (#FF00FF) background for later Pillow chroma-key removal, minimum 1024x1536.
3. {names['background']}: unoccupied {scene}, landscape 3:2 or wider, calm empty center for the leader overlay, no people, minimum 1536x1024.

After all three are copied, use Pillow to confirm files, dimensions and PNG format. If generation fails or reaches a usage limit, report the error and stop; do not substitute another service or an older placeholder."""


def validate(path: Path, kind: str) -> dict:
    with Image.open(path) as image:
        if image.format != "PNG":
            raise ValueError(f"{path}: expected PNG, got {image.format}")
        width, height = image.size
        if kind == "portrait" and (width < 768 or height < 768 or abs(width - height) > 16):
            raise ValueError(f"{path}: portrait must be square and at least 768px")
        if kind == "figure" and (width < 768 or height < 1024 or height <= width):
            raise ValueError(f"{path}: figure must be tall and at least 768x1024")
        if kind == "background" and (width < 1024 or height < 768 or width <= height):
            raise ValueError(f"{path}: background must be landscape and at least 1024x768")
    return {"output_file": str(path.relative_to(ORIGINALS.parent)),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "pixels": [width, height]}


def generate(key: str, cli: str) -> None:
    directory = ORIGINALS / key.lower()
    directory.mkdir(parents=True, exist_ok=True)
    images = {kind: directory / filename(key, kind)
              for kind in ("portrait", "figure", "background")}
    if all(path.is_file() for path in images.values()):
        for kind, path in images.items():
            validate(path, kind)
        print(f"SKIP {key}: three existing PNGs verified", flush=True)
        return
    if any(path.exists() for path in images.values()):
        raise RuntimeError(f"{key}: partial existing output; inspect before retrying")
    command = [cli, "exec", "--json", "--ignore-user-config", "--skip-git-repo-check",
               "-s", "danger-full-access", "-C", str(directory), "-"]
    environment = os.environ.copy()
    for variable in ("OPENAI_API_KEY", "AZURE_OPENAI_API_KEY", "ANTHROPIC_API_KEY", "GOOGLE_API_KEY"):
        environment.pop(variable, None)
    started = datetime.now(timezone.utc).isoformat()
    print(f"START {key} {started}", flush=True)
    flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
    result = subprocess.run(command, cwd=directory, env=environment, text=True, input=prompt(key),
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            creationflags=flags)
    (directory / "codex-cli-events.jsonl").write_text(result.stdout, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"{key}: Codex CLI exit {result.returncode}; inspect codex-cli-events.jsonl")
    missing = [str(path) for path in images.values() if not path.is_file()]
    if missing:
        raise RuntimeError(f"{key}: Codex CLI returned without generating {missing}; inspect codex-cli-events.jsonl")
    artifacts = [validate(path, kind) for kind, path in images.items()]
    completed = datetime.now(timezone.utc).isoformat()
    with (ORIGINALS.parent / "generation-log.jsonl").open("a", encoding="utf-8") as log:
        for artifact in artifacts:
            log.write(json.dumps({"leader": key, "tool": "official Codex CLI built-in image_gen (ChatGPT OAuth)",
                                  "started_utc": started, "completed_utc": completed,
                                  "prompt": prompt(key), **artifact}, ensure_ascii=False) + "\n")
    print(f"OK {key} {completed}: 3 PNGs", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--only", nargs="+", choices=SUBJECTS)
    args = parser.parse_args()
    cli = shutil.which("codex.cmd" if sys.platform == "win32" else "codex")
    if not cli:
        sys.exit("Codex CLI unavailable")
    for key in (args.only or SUBJECTS):
        generate(key, cli)


if __name__ == "__main__":
    main()
