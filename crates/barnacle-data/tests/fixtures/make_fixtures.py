import pickle
import struct
import zlib
from pathlib import Path

HERE = Path(__file__).parent


def ship(index, paper):
    entry = {
        "index": index,
        "typeinfo": {"type": "Ship", "nation": "USA", "species": "Battleship"},
    }
    if paper is not None:
        entry["isPaperShip"] = paper
    return entry


def parsable(index, name, level, paper):
    return {
        "id": 4181604048,
        "index": index,
        "name": name,
        "isPaperShip": paper,
        "level": level,
        "group": "upgradeable",
        "ShipUpgradeInfo": {},
        "typeinfo": {"type": "Ship", "nation": "USA", "species": "Battleship"},
    }


def game_params(root):
    return zlib.compress(pickle.dumps(root, protocol=2))[::-1]


def mo_file(messages):
    keys = sorted(messages)
    ids = [key.encode() for key in keys]
    strs = [messages[key].encode() for key in keys]
    count = len(keys)
    data_start = 28 + 16 * count
    offsets = []
    position = data_start
    for blob in ids + strs:
        offsets.append((len(blob), position))
        position += len(blob) + 1
    header = struct.pack("<7I", 0x950412DE, 0, count, 28, 28 + 8 * count, 0, 0)
    tables = b"".join(struct.pack("<2I", length, offset) for length, offset in offsets)
    return header + tables + b"".join(blob + b"\0" for blob in ids + strs)


ships = {
    "PASB008_Colorado": ship("PASB008", False),
    "PASB110_Vermont": ship("PASB110", True),
    "PAPT001_Torpedo": {
        "index": "PAPT001",
        "typeinfo": {"type": "Projectile", "nation": "USA", "species": "Torpedo"},
    },
}

(HERE / "mini_gameparams.data").write_bytes(game_params({"": ships}))
(HERE / "mini_gameparams_list_root.data").write_bytes(game_params([ships]))
(HERE / "mini_gameparams_missing_flag.data").write_bytes(
    game_params({"": {**ships, "PJSB018_Yamato": ship("PJSB018", None)}})
)
(HERE / "mini_gameparams_bad_wrapper.data").write_bytes(
    game_params({"": "not a dictionary", **ships})
)

colorado = {
    **parsable("PASB008", "PASB008_Colorado", 7, False),
    "A_Hull": {"model": "content/gameplay/usa/ship/battleship/ASB008_Colorado_1945/ASB008_Colorado_1945.model"},
}
vermont = parsable("PASB110", "PASB110_Vermont", 10, True)
torpedo = ships["PAPT001_Torpedo"]

(HERE / "catalog_ok.data").write_bytes(
    game_params({"": {"PASB008_Colorado": colorado, "PASB110_Vermont": vermont, "PAPT001_Torpedo": torpedo}})
)
(HERE / "catalog_unparsable.data").write_bytes(
    game_params({"": {"PASB008_Colorado": colorado, "PJSB018_Yamato": ship("PJSB018", False)}})
)
(HERE / "catalog_duplicate.data").write_bytes(
    game_params({"": {"PASB008_Colorado": colorado, "PASB008_Colorado_Copy": colorado}})
)
(HERE / "catalog_no_ships.data").write_bytes(game_params({"": {"PAPT001_Torpedo": torpedo}}))
(HERE / "mini_en_empty.mo").write_bytes(mo_file({"": "Content-Type: text/plain; charset=UTF-8\n"}))
(HERE / "mini_en.mo").write_bytes(
    mo_file(
        {
            "": "Content-Type: text/plain; charset=UTF-8\n",
            "IDS_PASB008": "Colorado",
            "IDS_PASB008_FULL": "Colorado",
            "IDS_PBSC210": "Goliath",
            "IDS_PGSC519": "Ägir",
            "IDS_PGSC519_FULL": "Ägir",
        }
    )
)
