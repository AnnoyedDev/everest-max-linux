# Protocole Mountain Everest Max (3282:0001) — tiré de Base Camp

Sources : `SDKDLL.dll` (désassemblé), `BaseCamp.Service.exe` et `BaseCamp.UI.dll` (métadonnées .NET + IL).

## Architecture de Base Camp (Windows)
- `Base Camp.exe` (Electron) affiche `BaseCamp.UI.exe` (ASP.NET, bundle single-file, SQLite `BaseCamp.db`).
- L'UI envoie des requêtes JSON (`{'Class':'Everest','MethodName':…}`) à `BaseCamp.Service.exe`, qui appelle `SDKDLL.dll`.
- Les actions « ordinateur » (programme, son, OBS, Spotify…) sont exécutées par le service quand la touche est pressée.

## Transport
Interface 3, page vendeur `0xFF00`, rapports de 64 octets sans report ID (sous Linux : préfixer `00` à l'écriture).
- **Commande** : rapport OUTPUT, réponse INPUT qui recopie l'en-tête (`11 14 …`). `FF AA` = refus. Délai 600 ms.
- **Événements** : rapport INPUT `01` = champ de bits de toutes les touches (D1–D4 = octet 42, bits 1–4).
  `02`–`0A` = autres événements, à ne pas confondre avec une réponse.
- **Transferts d'image** : feature reports de 64 octets (HIDIOCSFEATURE/HIDIOCGFEATURE). Dans la réponse, `[2]` = écho de la commande et `[3]` = statut :
  `FA` OK, `FB` occupé (réessayer), `FC` OK, `FE` erreur.

## Éclairage
| Commande | Paquet |
|---|---|
| SwitchProfile(profil 1-5, slot 0-8) | `14 00 00 00 profil slot` |
| ChangeEffect(EffData 62 o) | `14 2C` + `effet byAll vitesse lum. randColor direction FF` + 3×RGB + RGB fond + 43×00 |
| ChangeCustomizeStatic | 7 × `14 2C 00 01 n 4B 00` + 57 o (126 RGB indexés par matrixId) |
| SaveFlash(slot) | `13 55 00 00 slot` |

Effets : Statique 0, Respiration 1, Réactif 3, Vague 4, Yeti 6, Tornade 7, Matrix 9, Personnalisé 10 (byAll=1), Éteint 12.
randColor : 0 une couleur, 16 deux couleurs, 2 arc-en-ciel.

## Touches (stockées dans le clavier)
| Commande | Paquet |
|---|---|
| ChangeKey(src, valeur) | `14 20 src 00 val_lo val_hi`. src = DLLKeyId (`rust/data/keys.json`) |
| ChangeShortcutKey(src, touche, mods) | `14 21 src 00 touche mods` (Ctrl 1, Maj 2, Alt 4, Win 8) |
| Macro : en-tête | `14 18 mode 00 répétitions` (mode : 1 une fois, 4 maintenue, 3 bascule — libellés non confirmés) |
| Macro : données | `15 src nb suite(0/1)` + jusqu'à 15 × `touche page<<4|action délai_lo délai_hi`, terminé par `FF FF 00 00` |
| SetLaunchContent | `17 src len étape(0/1/2/3)` + 60 o (contenu « lancement » lu par le service) |

Valeurs de ChangeKey : 0 désactivée, 255 par défaut, DLLKeyId d'une autre touche, multimédia 180–188,
souris 257–261/304/305, profils 160–164/210–212, extinction 128, veille 129, calculatrice 195 (`rust/data/function_values.json`).
Événements de macro : action 2 appui, 1 relâche, 3 molette ; page 0 clavier, 1 souris (1 gauche, 2 droit, 3 milieu, 49/48 molette).

## Images (touches écran D1–D4, dock)
Format RGB565 little-endian, lignes de haut en bas.
1. `AA 55 21 type` (type 3 = dock, 4 = pavé) -> réponse `21 FA`.
2. `AA 55 10 taille[3] somme[2] 00 00 mode banque sous` -> réponse `10 FA`. somme = addition de tous les octets, sur 16 bits.
   Pavé : mode 2, banque = `FW_EXTEND_INFO.byMMDockShowProfile[profil-1]` (1 pour le profil 1), sous = touche 0–3. Les images font 72×72.
   Dock : mode 2, banque 1 = écran 240×204, banques 2–6 = icônes 128×32.
3. Blocs de 64 o en feature report ; la réponse donne le statut + le nombre d'octets reçus `[4..7]`. Au dernier bloc : somme `[7..9]`, attente ~0,6 s.
4. En cas d'échec : `1D 00` (sortie du mode transfert), puis nouvel essai.

## Dock média
| Commande | Paquet |
|---|---|
| SetClockInfo | `11 84 00 01 00 00 mois jour heure min sec f1 f2` |
| SetVolumeInfo | `11 83 00 00 volume(0-100)` |
| SetPCInfo(type 0-5, valeur) | `11 81 type 00 val_lo val_hi` (sens des types non confirmé, non utilisé) |
| GetExtendInfo | `11 14` -> FW_EXTEND_INFO à partir de l'octet 4 |
| EraseSectorMem | `1B …` — **ne pas utiliser** |

## Compléments (transformations faites par le SDK avant l'envoi)
- **ChangeEffect** (Statique 0, Respiration 1, Réactif 3, Yeti 6, Matrix 9, Éteint 12) : sens et largeur forcés à `FF`.
  Statique : vitesse `FF`, randColor 0 ou 2. Éteint : randColor `FF`.
- **Vitesse** (0–100 dans l'UI) convertie en code firmware : Vague/Tornade 10/9/8/7/6, Respiration/Réactif 5/4/3/1/0,
  Matrix 20/15/10/5/0, Yeti 10/7/5/3/0 (seuils 12/37/62/87).
- **ChangeBlockEffect** (Vague 4, Tornade 7) : `effet byAll vitesse lum. rand sens largeur nbBlocs` + couleurs `niveau R G B`.
  Vague : sens 6/2/4/0 ; une couleur -> 1 bloc niveau 100 ; deux couleurs -> 4 blocs 25/50/75/100 ; arc-en-ciel -> largeur 2, niveau `FF`.
  Tornade : sens 10 (horaire) ou 9.
- **Personnalisé** : seulement ChangeCustomizeStatic (le SDK n'envoie pas d'EffData pour l'effet 10). ~47 images/s possibles.

## Configuration étendue du dock (`11 14` lecture, `11 14 00 01` + 29 octets écriture)
| Octet | Champ |
|---|---|
| 1 | menus de la molette, un bit par menu : 7 perso, 6 APM, 5 PC info, 4 luminosité, 3 volume, 2 éclairage, 1 profil, 0 horloge |
| 3–5 | couleur des menus |
| 6 | écran de veille : type<<4, bit1 extinction, bit0 veille. Types : 0 image, 1/2 horloge, 3 minuteur, 4 chrono, 7 volume, 8 luminosité, 9 CPU, 10 GPU, 11 disque, 12 RAM, 13 internet, 14 APM |
| 7–8 / 9–10 | délai de veille / d'extinction en secondes (≤ 1800) |
| 11–15 | profils : actifs `n` d'abord, puis désactivés `0x10\|n` |

## PC info
`11 81 type 00 val_lo val_hi` envoyé chaque seconde par l'ordinateur : 0 CPU %, 1 GPU %, 2 disque %, 3 internet, 4 RAM %, 5 APM (≤ 999).
Sans ce flux, le dock affiche 0.
