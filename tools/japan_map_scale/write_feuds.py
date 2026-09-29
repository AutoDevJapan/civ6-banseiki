"""Generate Data/JAC_Japan_Feuds.xml: the 1534 feuds as a visible AI-opinion penalty.

Official mechanism: a trait attached to the AI's civilization carries
MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER modifiers whose SubjectRequirementSet tests the other
player (Base/Assets/Gameplay/Data/Agendas.xml:451, :813; the reason text comes from
SimpleModifierDescription). InitialValue without ReductionTurns never decays while the subject
matches (Agendas.xml:2247 AGENDA_SAME_GOVERNMENT). The subject test is
REQUIREMENT_PLAYER_TYPE_MATCHES + CivilizationType (DLC/GranColombia_Maya/Data/GranColombia_Maya_Civilizations.xml:403,414).

The reason only reaches the diplomacy screen through a ModifierStrings row (Context "Sample", Text
LOC_TOOLTIP_SAMPLE_DIPLOMACY_ALL = "{diplomaticReason}", filled with SimpleModifierDescription):
Base/Assets/Gameplay/Data/Agendas.xml:2296-2370 has one per agenda modifier. Without it
GetDiplomaticModifiers returns LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON ("Unknown Reason"), which
Base/Assets/UI/DiplomacyActionView.lua:915 shows in grey.

The engine also hides every reason (official ones too) while the viewer has access level None:
"If you gain Limited Diplomatic Visibility, you will find out some information about their opinion
of you" (Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:771). Freshly met houses sit at None
(no delegation yet, and the Sengoku tree moves TECH_PRINTING's +1 to a later tech), so on these maps
EVERY major gets +1 access on every met player the way Printing grants it: MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY
with Amount/Source/SourceType (Base/Assets/Gameplay/Data/Technologies.xml:546-616), attached to one
trait that all 22 civilizations carry, like Mongolia's (DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:178-179,
:352-353, :620-629). The source row must name what grants it: every official row carries one gate
(PrereqTech, TraitType, GreatPersonIndividualType, Trader/Delegate/Ally/Spy, or the XP1 TradePostTrait;
Base/Assets/Gameplay/Data/Gossip.xml:75-81, Expansion1_Civilizations_Major.xml:1100-1105). A bare row (no gate)
gave access level 0 live on 2026-09-29. The row copies Catherine de Medici's SOURCE_TRAIT, the official
"+1 access on every met civ from turn 1" (Gossip.xml:78: TraitType + FromCitizen; Base/Assets/Gameplay/Data/Leaders.xml:1088,
:1698-1701, :4011-4025).
Opinion values are untouched. The Lua script has all majors meet each other at game start.
"""

from __future__ import annotations

import json
import xml.etree.ElementTree as ET

from generate import MOD, ROOT
from write_support import write_xml

DATA = ROOT / "documents/spec/data/japan-1534-relations.json"
PENALTY = -10  # comparable to a denounced friend / different-government reason (-6); never decays
TEXT_TAG = "LOC_JAC_DIPLO_MODIFIER_1534_FEUD"
SAMPLE_TAG = "LOC_TOOLTIP_SAMPLE_DIPLOMACY_ALL"  # "{diplomaticReason}", as every official agenda modifier uses
ACCESS_MODIFIER = "JAC_1534_DIPLO_VISIBILITY"
ACCESS_SOURCE = "SOURCE_JAC_1534_ACCESS"
ACCESS_TRAIT = "TRAIT_JAC_1534_ACCESS"


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
                 "RequirementSets", "Requirements", "RequirementArguments", "RequirementSetRequirements", "ModifierStrings",
                 "DiplomaticVisibilitySources"):
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
        ET.SubElement(t["ModifierStrings"], "Row", ModifierId=mod, Context="Sample", Text=SAMPLE_TAG)
        ET.SubElement(t["TraitModifiers"], "Row", TraitType=f"TRAIT_JAC_1534_FEUD_{a}", ModifierId=mod)
    # +1 access level (None -> Limited) toward every met player, for all majors, so no reason is "Unknown Reason".
    ET.SubElement(t["Types"], "Row", Type=ACCESS_TRAIT, Kind="KIND_TRAIT")
    ET.SubElement(t["Traits"], "Row", TraitType=ACCESS_TRAIT)
    for c in json.loads(DATA.read_text(encoding="utf-8"))["factions"]:
        ET.SubElement(t["CivilizationTraits"], "Row", CivilizationType=f"CIVILIZATION_JAC_{c}", TraitType=ACCESS_TRAIT)
    ET.SubElement(t["Modifiers"], "Row", ModifierId=ACCESS_MODIFIER, ModifierType="MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY")
    for name, value in (("Amount", "1"), ("Source", ACCESS_SOURCE), ("SourceType", "DIPLO_SOURCE_ALL_NAMES")):
        ET.SubElement(t["ModifierArguments"], "Row", ModifierId=ACCESS_MODIFIER, Name=name, Value=value)
    ET.SubElement(t["TraitModifiers"], "Row", TraitType=ACCESS_TRAIT, ModifierId=ACCESS_MODIFIER)
    ET.SubElement(t["DiplomaticVisibilitySources"], "Row", VisibilitySourceType=ACCESS_SOURCE,
                  Description="LOC_JAC_VIZSOURCE_1534_ACCESS", ActionDescription="LOC_JAC_VIZSOURCE_ACTION_1534_ACCESS",
                  GossipString="LOC_JAC_GOSSIP_SOURCE_1534_ACCESS", FromCitizen="true", TraitType=ACCESS_TRAIT)
    return root


def main() -> None:
    write_xml(MOD / "Data" / "JAC_Japan_Feuds.xml", build())


if __name__ == "__main__":
    main()
