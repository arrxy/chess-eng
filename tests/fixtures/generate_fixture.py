"""Build the famous-games test fixture (tests/fixtures/famous_games.tsv).

Usage:
    1. Download PGN collections into /tmp/pgns/, e.g.
       curl -o /tmp/pgns/WorldChamp1972.pgn https://www.pgnmentor.com/events/WorldChamp1972.pgn
    2. pip install chess
    3. python generate_fixture.py famous_games.tsv

For every game: replay the mainline with python-chess (independent legality
check), regenerate SAN so check/mate markers are normalized, and record the
final position and status. Output TSV:
    name <TAB> result <TAB> final status <TAB> final board (FEN field) <TAB> SAN moves
"""
import glob
import sys
import chess
import chess.pgn

out_path = sys.argv[1]
records = []
stats = {"castle": 0, "ep": 0, "promo": 0, "check": 0, "mate": 0, "stalemate": 0}

# Short famous games that actually end in checkmate (WCC games end by
# resignation, so the corpus needs these to exercise mate detection).
FAMOUS_MATES = """
[Event "Opera Game, Paris"][Site "?"][Date "1858.??.??"][Round "?"][White "Morphy, Paul"][Black "Duke Karl / Count Isouard"][Result "1-0"]
1.e4 e5 2.Nf3 d6 3.d4 Bg4 4.dxe5 Bxf3 5.Qxf3 dxe5 6.Bc4 Nf6 7.Qb3 Qe7 8.Nc3 c6 9.Bg5 b5 10.Nxb5 cxb5 11.Bxb5+ Nbd7 12.O-O-O Rd8 13.Rxd7 Rxd7 14.Rd1 Qe6 15.Bxd7+ Nxd7 16.Qb8+ Nxb8 17.Rd8# 1-0

[Event "Immortal Game, London"][Site "?"][Date "1851.??.??"][Round "?"][White "Anderssen, Adolf"][Black "Kieseritzky, Lionel"][Result "1-0"]
1.e4 e5 2.f4 exf4 3.Bc4 Qh4+ 4.Kf1 b5 5.Bxb5 Nf6 6.Nf3 Qh6 7.d3 Nh5 8.Nh4 Qg5 9.Nf5 c6 10.g4 Nf6 11.Rg1 cxb5 12.h4 Qg6 13.h5 Qg5 14.Qf3 Ng8 15.Bxf4 Qf6 16.Nc3 Bc5 17.Nd5 Qxb2 18.Bd6 Bxg1 19.e5 Qxa1+ 20.Ke2 Na6 21.Nxg7+ Kd8 22.Qf6+ Nxf6 23.Be7# 1-0

[Event "Evergreen Game, Berlin"][Site "?"][Date "1852.??.??"][Round "?"][White "Anderssen, Adolf"][Black "Dufresne, Jean"][Result "1-0"]
1.e4 e5 2.Nf3 Nc6 3.Bc4 Bc5 4.b4 Bxb4 5.c3 Ba5 6.d4 exd4 7.O-O d3 8.Qb3 Qf6 9.e5 Qg6 10.Re1 Nge7 11.Ba3 b5 12.Qxb5 Rb8 13.Qa4 Bb6 14.Nbd2 Bb7 15.Ne4 Qf5 16.Bxd3 Qh5 17.Nf6+ gxf6 18.exf6 Rg8 19.Rad1 Qxf3 20.Rxe7+ Nxe7 21.Qxd7+ Kxd7 22.Bf5+ Ke8 23.Bd7+ Kf8 24.Bxe7# 1-0

[Event "Fool's Mate"][Site "?"][Date "????.??.??"][Round "?"][White "NN"][Black "NN"][Result "0-1"]
1.f3 e5 2.g4 Qh4# 0-1

[Event "Scholar's Mate"][Site "?"][Date "????.??.??"][Round "?"][White "NN"][Black "NN"][Result "1-0"]
1.e4 e5 2.Bc4 Nc6 3.Qh5 Nf6 4.Qxf7# 1-0

[Event "Legal's Mate, Paris"][Site "?"][Date "1750.??.??"][Round "?"][White "Legall de Kermeur"][Black "Saint Brie"][Result "1-0"]
1.e4 e5 2.Nf3 d6 3.Bc4 Bg4 4.Nc3 g6 5.Nxe5 Bxd1 6.Bxf7+ Ke7 7.Nd5# 1-0
"""

with open("/tmp/pgns_extra.pgn", "w") as f:
    f.write(FAMOUS_MATES)

for path in sorted(glob.glob("/tmp/pgns/*.pgn")) + ["/tmp/pgns_extra.pgn"]:
    with open(path, encoding="latin-1") as f:
        while True:
            game = chess.pgn.read_game(f)
            if game is None:
                break
            h = game.headers
            if "FEN" in h or "SetUp" in h or h.get("Variant", "Standard") != "Standard":
                continue
            result = h.get("Result", "*")
            if result not in ("1-0", "0-1", "1/2-1/2"):
                continue
            board = game.board()
            sans = []
            ok = True
            for mv in game.mainline_moves():
                if not board.is_legal(mv):
                    ok = False
                    break
                if board.is_castling(mv):
                    stats["castle"] += 1
                if board.is_en_passant(mv):
                    stats["ep"] += 1
                if mv.promotion:
                    stats["promo"] += 1
                san = board.san(mv)
                board.push(mv)
                if san.endswith("#"):
                    stats["mate"] += 1
                elif san.endswith("+"):
                    stats["check"] += 1
                sans.append(san)
            if not ok or len(sans) < 2:
                continue
            name = "{} {} {}-{}".format(
                h.get("Event", "?"), h.get("Round", "?"),
                h.get("White", "?").split(",")[0], h.get("Black", "?").split(",")[0],
            )
            if board.is_checkmate():
                status = "checkmate"
            elif board.is_stalemate():
                status = "stalemate"
                stats["stalemate"] += 1
            elif board.is_check():
                status = "check"
            else:
                status = "ongoing"
            placement = board.fen().split()[0]
            records.append("\t".join([name, result, status, placement, " ".join(sans)]))

with open(out_path, "w") as f:
    f.write("# Famous games fixture: World Championship matches 1886-2023 (pgnmentor.com)\n")
    f.write("# plus classic miniatures ending in checkmate. Validated with python-chess.\n")
    f.write("# name <TAB> result <TAB> final status <TAB> final position (FEN board field) <TAB> SAN moves\n")
    f.write("\n".join(records) + "\n")

print(f"games={len(records)} stats={stats}")
