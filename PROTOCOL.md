# Protocole du Mountain Everest Max

[English](PROTOCOL.en.md) · **Français**

Les paquets USB que Base Camp échange avec le clavier Everest Max (identifiant USB `3282:0001`). Tout vient du désassemblage de `SDKDLL.dll` et de la lecture de `BaseCamp.Service.exe` et `BaseCamp.UI.dll` (métadonnées .NET et IL).

## Comment lire les paquets

Les octets entre accents graves sont en hexadécimal (`14 2C`), les autres nombres en décimal. Un paquet fait toujours 64 octets, et tout ce qui n'est pas écrit vaut `00`. `val_lo val_hi` désigne une valeur sur 16 bits, octet de poids faible en premier. Les noms de commandes (ChangeEffect, SaveFlash…) sont ceux des fonctions du SDK.

## Comment Base Camp est construit

Sous Windows, `Base Camp.exe` est une fenêtre Electron qui affiche `BaseCamp.UI.exe`, une appli ASP.NET livrée en un seul fichier avec une base SQLite `BaseCamp.db`. L'interface envoie des requêtes JSON (`{'Class':'Everest','MethodName':…}`) à `BaseCamp.Service.exe`, et ce service appelle `SDKDLL.dll` pour parler au clavier.

Les actions « ordinateur » (programme, son, OBS, Spotify…) sont exécutées par le service au moment où la touche est pressée.

## Transport USB

Tout passe par l'interface HID numéro 3 (page d'usage constructeur `0xFF00`), avec des rapports de 64 octets sans report ID. Sous Linux il faut donc écrire 65 octets : un `00`, puis le paquet.

| Échange | Fonctionnement |
|---|---|
| Commande | Rapport OUTPUT. Le clavier répond par un rapport INPUT qui reprend l'en-tête de la commande (`11 14 …`). Une réponse `FF AA` est un refus. |
| Événement | Rapport INPUT que le clavier envoie de lui-même. `01` : champ de bits de toutes les touches (D1 à D4 : octet 42, bits 1 à 4). `02` à `0A` : autres événements, à ne pas prendre pour une réponse. |
| Transfert d'image | Feature reports de 64 octets (`HIDIOCSFEATURE` / `HIDIOCGFEATURE`). Dans la réponse, l'octet 2 répète la commande et l'octet 3 donne le statut : `FA` OK, `FB` occupé (réessayer), `FC` OK, `FE` erreur. |

L'appli attend la réponse à une commande 600 ms au plus. Trois commandes ont droit à un délai plus long : 950 ms pour SwitchProfile, 1,3 s pour SaveFlash et 2,8 s pour l'écriture de la configuration du dock.

## Éclairage

Pour changer d'effet, l'appli envoie SwitchProfile, puis ChangeEffect (ou ChangeCustomizeStatic pour l'effet Personnalisé), puis SaveFlash si le clavier doit garder l'effet en mémoire.

| Commande | Paquet |
|---|---|
| SwitchProfile(profil 1 à 5, slot 0 à 8) | `14 00 00 00 profil slot` |
| ChangeEffect(EffData) | `14 2C` + 62 octets d'EffData |
| ChangeCustomizeStatic | 7 paquets `14 2C 00 01 n 4B 00` + 57 octets |
| SaveFlash(slot) | `13 55 00 00 slot` |

Chaque effet a un numéro, placé dans EffData, et un slot, passé à SwitchProfile et SaveFlash :

| Effet | Numéro | Slot |
|---|---|---|
| Statique | 0 | 0 |
| Respiration | 1 | 3 |
| Réactif | 3 | 4 |
| Vague | 4 | 1 |
| Yeti | 6 | 7 |
| Tornade | 7 | 2 |
| Matrix | 9 | 5 |
| Personnalisé | 10 | 6 |
| Éteint | 12 | 8 |

### EffData pour Statique, Respiration, Réactif, Yeti, Matrix et Éteint

| Octet | Contenu |
|---|---|
| 0 | numéro de l'effet |
| 1 | byAll |
| 2 | vitesse, en code firmware (tableau plus bas). `FF` pour Statique. |
| 3 | luminosité |
| 4 | randColor : 0 une couleur, 16 deux couleurs, 2 arc-en-ciel. Statique accepte 0 ou 2. `FF` pour Éteint. |
| 5 | sens, toujours `FF` pour ces effets |
| 6 | largeur, toujours `FF` pour ces effets |
| 7 à 15 | trois couleurs RGB |
| 16 à 18 | couleur de fond RGB |
| 19 à 61 | `00` |

### EffData pour Vague et Tornade

Le SDK appelle cette variante ChangeBlockEffect. Les couleurs y sont rangées par blocs.

| Octet | Contenu |
|---|---|
| 0 à 4 | effet, byAll, vitesse, luminosité, randColor, comme ci-dessus |
| 5 | sens |
| 6 | largeur |
| 7 | nombre de blocs |
| 8 et suivants | un bloc de 4 octets par couleur : `niveau R G B` |

Pour Vague, le sens vaut 6, 2, 4 ou 0. Avec une couleur, il y a 1 bloc de niveau 100. Avec deux couleurs, 4 blocs de niveaux 25, 50, 75 et 100. En arc-en-ciel, la largeur vaut 2 et le niveau `FF`.

Pour Tornade, le sens vaut 10 (horaire) ou 9.

### Vitesse

L'interface règle la vitesse de 0 à 100. Le SDK la convertit en code firmware selon l'effet :

| Vitesse dans l'interface | 0 à 12 | 13 à 37 | 38 à 62 | 63 à 87 | 88 à 100 |
|---|---|---|---|---|---|
| Vague, Tornade | 10 | 9 | 8 | 7 | 6 |
| Respiration, Réactif | 5 | 4 | 3 | 1 | 0 |
| Matrix | 20 | 15 | 10 | 5 | 0 |
| Yeti | 10 | 7 | 5 | 3 | 0 |

### Couleur par touche

Pour l'effet Personnalisé (numéro 10, byAll = 1), le SDK envoie uniquement ChangeCustomizeStatic, sans EffData. Les données sont les couleurs RGB de 126 touches, rangées par matrixId (le champ `matrix` de `rust/data/keys.json`). Ces 378 octets sont découpés en 7 paquets de 57 octets, et `n` est le numéro du paquet, de 0 à 6. Le clavier accepte environ 47 images par seconde.

## Touches

Ces réglages sont stockés dans le clavier. `src` est le DLLKeyId de la touche à modifier (le champ `dll` de `rust/data/keys.json`).

| Commande | Paquet |
|---|---|
| ChangeKey(src, valeur) | `14 20 src 00 val_lo val_hi` |
| ChangeShortcutKey(src, touche, mods) | `14 21 src 00 touche mods` |
| En-tête de macro | `14 18 mode 00 répétitions` |
| Données de macro | `15 src nb suite` + entrées de 4 octets |
| SetLaunchContent | `17 src len étape` + 60 octets |

### Valeurs de ChangeKey

| Valeur | Résultat |
|---|---|
| 0 | touche désactivée |
| 255 | comportement par défaut |
| DLLKeyId d'une autre touche | la touche devient cette autre touche |
| 180 à 188 | multimédia |
| 257 à 261, 304, 305 | souris |
| 160 à 164, 210 à 212 | profils |
| 128 | extinction |
| 129 | veille |
| 195 | calculatrice |

Toutes les valeurs sont dans `rust/data/function_values.json`.

### Raccourcis

`mods` additionne les modificateurs : Ctrl 1, Maj 2, Alt 4, Win 8.

### Macros

L'en-tête donne le mode de lecture : 1 une fois, 4 tant que la touche est maintenue, 3 bascule. Ces libellés ne sont pas confirmés.

Viennent ensuite les paquets de données. `nb` est le nombre d'entrées du paquet, 15 au plus, et `suite` vaut 0 dans le premier paquet, 1 dans les suivants. Une entrée fait 4 octets :

```
touche   (page << 4) | action   délai_lo   délai_hi
```

| Champ | Valeurs |
|---|---|
| action | 2 appui, 1 relâche, 3 molette |
| page | 0 clavier, 1 souris |
| touche, pour la souris | 1 gauche, 2 droit, 3 milieu, 49 et 48 molette |

La macro se termine par l'entrée `FF FF 00 00`, comptée dans `nb`.

### Contenu de lancement

Dans SetLaunchContent, `étape` vaut 0, 1, 2 ou 3. Les 60 octets portent le contenu « lancement » que le service lit.

## Images

Les touches écran D1 à D4 et le dock reçoivent leurs images en RGB565 little-endian, lignes de haut en bas. Une touche écran fait 72×72, l'écran du dock 240×204 et une icône du dock 128×32.

Les étapes 1 à 3 passent par des feature reports. Une réponse notée `21 FA` se lit aux octets 2 et 3.

1. Envoyer `AA 55 21 type`, avec le type 3 pour le dock ou 4 pour le pavé. Réponse attendue : `21 FA`.
2. Envoyer `AA 55 10 taille[3] somme[2] 00 00 mode banque sous`. Réponse attendue : `10 FA`. La taille tient sur 3 octets. La somme est l'addition de tous les octets de l'image, gardée sur 16 bits.
3. Envoyer l'image par blocs de 64 octets. Chaque réponse donne le statut et le nombre d'octets reçus (octets 4 à 6). Après le dernier bloc, attendre environ 0,6 s : la réponse contient alors la somme aux octets 7 et 8.
4. En cas d'échec, envoyer la commande `1D 00` pour sortir du mode transfert, puis recommencer.

Pour le pavé, le mode vaut 2, la banque vaut `FW_EXTEND_INFO.byMMDockShowProfile[profil-1]` (1 pour le profil 1) et `sous` est la touche, de 0 à 3. Pour le dock, le mode vaut 2, la banque 1 est l'écran et les banques 2 à 6 sont les icônes.

## Dock média

| Commande | Paquet |
|---|---|
| SetClockInfo | `11 84 00 01 00 00 mois jour heure min sec f1 f2` (l'appli envoie `00 00` pour f1 et f2) |
| SetVolumeInfo | `11 83 00 00 volume`, volume de 0 à 100 |
| SetPCInfo(type, valeur) | `11 81 type 00 val_lo val_hi` |
| GetExtendInfo | `11 14`. La réponse contient FW_EXTEND_INFO à partir de l'octet 4. |
| Écriture de FW_EXTEND_INFO | `11 14 00 01` + 29 octets |
| EraseSectorMem | `1B …` **Ne pas utiliser.** |

### Infos PC

L'ordinateur envoie SetPCInfo chaque seconde, une fois par type. Sans ce flux, le dock affiche 0.

| Type | Valeur |
|---|---|
| 0 | CPU, en % |
| 1 | GPU, en % |
| 2 | disque, en % |
| 3 | internet |
| 4 | RAM, en % |
| 5 | APM, 999 au plus |

### Configuration étendue (FW_EXTEND_INFO)

Ce bloc de 29 octets règle les menus, l'écran de veille et les profils.

| Octet | Champ |
|---|---|
| 1 | menus de la molette, un bit par menu |
| 3 à 5 | couleur des menus |
| 6 | écran de veille : type dans les 4 bits de poids fort, bit 1 extinction, bit 0 veille |
| 7 et 8 | délai de veille en secondes, 1800 au plus |
| 9 et 10 | délai d'extinction en secondes, 1800 au plus |
| 11 à 15 | profils : d'abord les numéros des profils actifs, puis ceux des profils désactivés sous la forme `0x10 + n` |

Bits des menus : 7 perso, 6 APM, 5 PC info, 4 luminosité, 3 volume, 2 éclairage, 1 profil, 0 horloge.

Types d'écran de veille : 0 image, 1 et 2 horloge, 3 minuteur, 4 chrono, 7 volume, 8 luminosité, 9 CPU, 10 GPU, 11 disque, 12 RAM, 13 internet, 14 APM.
