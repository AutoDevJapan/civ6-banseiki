"""Generate Data/JAC_Japan_Feuds.xml: the 1534 feuds as a visible AI-opinion penalty.

Official mechanism: a trait attached to the AI's civilization carries
MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER modifiers whose SubjectRequirementSet tests the other
player (Base/Assets/Gameplay/Data/Agendas.xml:451, :813; the reason text comes from
SimpleModifierDescription). InitialValue without ReductionTurns never decays while the subject
matches (Agendas.xml:2247 AGENDA_SAME_GOVERNMENT). The subject test is
REQUIREMENT_PLAYER_TYPE_MATCHES + CivilizationType (DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:403,414).
"""

from __future__ import annotations

import json
import xml.etree.ElementTree as ET

from generate import MOD, ROOT
from write_support import write_xml

DATA = ROOT / "documents/spec/data/japan-1534-relations.json"
PENALTY = -10  # comparable to a denounced friend / different-government reason (-6); never decays
TEXT_TAG = "LOC_JAC_DIPLO_MODIFIER_1534_FEUD"


def feud_pairs() -> list[tuple[str, str]]:
    """Hostility rows in force on the start date (same rule as the JSON apply_rule)."""
    data = json.loads(DATA.read_text(encoding="utf-8"))
    start = data["start_date"]

    def key(value, missing):
        if value is None:
            return (missing * 100,) * 3
        parts = [int(p) for p in value.split("-")]
        return tuple(parts + [missing] * (3 - len(parts)))

    return [(r["a"], r["b"]) for r in data["relations"]
            if r["type"] == "hostility" and r["confidence"] != "low"
            and key(r["from"], 0) <= key(start, 0) < key(r["to"], 99)]


def build() -> ET.Element:
    pairs = feud_pairs()
    directed = [(a, b) for x, y in pairs for a, b in ((x, y), (y, x))]
    civs = sorted({a for a, _ in directed})
    root = ET.Element("GameInfo")

    def table(name):
        return ET.SubElement(root, name)

    for name in ("Types", "Traits", "CivilizationTraits", "Modifiers", "ModifierArguments", "TraitModifiers",
                 "RequirementSets", "Requirements", "RequirementArguments", "RequirementSetRequirements"):
        table(name)
    t = {child.tag: child for child in root}
    for c in civs:
        trait = f"TRAIT_JAC_1534_FEUD_{c}"
        ET.SubElement(t["Types"], "Row", Type=trait, Kind="KIND_TRAIT")
        ET.SubElement(t["Traits"], "Row", TraitType=trait)
        ET.SubElement(t["CivilizationTraits"], "Row", CivilizationType=f"CIVILIZATION_JAC_{c}", TraitType=trait)
        ET.SubElement(t["RequirementSets"], "Row", RequirementSetId=f"JAC_1534_PLAYER_IS_{c}",
                      RequirementSetType="REQUIREMENTSET_TEST_ALL")
        ET.SubElement(t["Requirements"], "Row", RequirementId=f"JAC_1534_REQ_IS_{c}",
                      RequirementType="REQUIREMENT_PLAYER_TYPE_MATCHES")
        ET.SubElement(t["RequirementArguments"], "Row", RequirementId=f"JAC_1534_REQ_IS_{c}",
                      Name="CivilizationType", Value=f"CIVILIZATION_JAC_{c}")
        ET.SubElement(t["RequirementSetRequirements"], "Row", RequirementSetId=f"JAC_1534_PLAYER_IS_{c}",
                      RequirementId=f"JAC_1534_REQ_IS_{c}")
    for a, b in directed:
        mod = f"JAC_1534_FEUD_{a}_{b}"
        ET.SubElement(t["Modifiers"], "Row", ModifierId=mod, ModifierType="MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER",
                      SubjectRequirementSetId=f"JAC_1534_PLAYER_IS_{b}")
        for name, value in (("InitialValue", str(PENALTY)), ("SimpleModifierDescription", TEXT_TAG)):
            ET.SubElement(t["ModifierArguments"], "Row", ModifierId=mod, Name=name, Value=value)
        ET.SubElement(t["TraitModifiers"], "Row", TraitType=f"TRAIT_JAC_1534_FEUD_{a}", ModifierId=mod)
    return root


def main() -> None:
    write_xml(MOD / "Data" / "JAC_Japan_Feuds.xml", build())


if __name__ == "__main__":
    main()
