# Sphere Burrower - Game Design Document

## Executive Summary

**Game Title**: Sphere Burrower (Working Title)

**Genre**: Contemplative Action-Adventure with Voxel Terrain Manipulation

**Core Concept**: 
A meditative exploration game where players control a spherical rodent that shapes the world through digging, creating burrows and underground homes while living in harmony with procedural forest ecosystems.

**Target Experience**: 
- Primary: Satisfaction of carving out cozy spaces in nature
- Secondary: Tactile pleasure of voxel terrain manipulation
- Tertiary: Discovery and collection in a living world

**Platform**: Gamepad-focused (PC/Console primary, iOS future consideration)

---

## Core Character & Progression

### The Spherical Rodent

**Form Factor**: Perfect sphere when rolling, unfolds limbs when still

### Progression Overview
- Episodes act as milestone checklists. Completing a milestone and sleeping in a safe burrow finalizes progress and autosaves.
- Rewards are systemic: unlock recipes, advance the workshop, or spend Wildgrit on invisible skill upgrades. No character model changes or size progression.

### Episode Milestones

Progress locks in by completing episode goals and sleeping in a safe burrow:

1. **First Burrow** - Dig 50 blocks, find safe space
2. **Mineral Discovery** - Find 3 different minerals
3. **Moonleaf Night** - Harvest first moonleaf
4. **Deep Delver** - Reach bedrock layer
5. **Radgum Farmer** - Establish tree grove
6. **Home Builder** - Create 5-room burrow
7. **Gear Crafter** - Build first craft station
8. **Lava Walker** - Cross molten chamber

---

## Control Scheme

### Gamepad Layout
```
LEFT STICK: Movement/Rolling/Climbing
RIGHT STICK: Camera Control
RT: UNIVERSAL DIG (tap for burst, hold for continuous)
LT: Sprint
A: Jump/Bounce
B: Eat/Interact
Y: Camera Mode Toggle (Close/Mid/Far)
X: Drop/Place Item
D-PAD: Quick item selection / Camera toggle
Start: Pause/Inventory
```

### Core Mechanics

#### Universal Digging (RT)
- **Tap RT**: "Ball Burst" - Quick roll attack forward, breaks 1-2 soft blocks
- **Hold RT**: Continuous ball roll with physics-based momentum
- Works in all contexts: ground, climbing, swimming
- Creates satisfying particle effects and sounds
- Material-dependent feedback (soft vs hard)

#### Automatic Climbing
- Triggered when hitting climbable surfaces (>70° angle)
- Trees, rocky cliffs, thick roots, dig-carved handholds
- Left stick moves along surface, RT creates new grip points
- Smooth transitions between rolling and climbing

#### Three-Camera System
```
CLOSE (FPS): Rodent's perspective, intimate digging
MID (TPS): See character clearly, general play
FAR (TPS): Overview for navigation and planning
```

---

## Material & Crafting System

### The Eight Minerals

**Common (Surface to Mid-depth)**:
1. **Copperstone** - Orange-red, warm to touch
2. **Ironvein** - Dark grey with rust, magnetic
3. **Silverflow** - Bright silver, flows like mercury

**Uncommon (Deeper)**:
4. **Goldnugget** - Classic gold, never tarnishes
5. **Crystalquartz** - Clear/purple, rings when struck
6. **Voidstone** - Pure black, absorbs light

**Rare (Special locations)**:
7. **Skyglass** - Translucent blue, shows clouds within
8. **Heartfire** - Pulsing red, warm glow

### Meta-Materials

**Moonleaf** (Fur Enhancement):
- Silvery-white leaves that glow at night
- Only grows in moonlit clearings
- Grows over 3 nights, wilts if not collected
- Can be consumed for temporary rolling hazard resistance or converted into high-yield Wildgrit

**Radgum** (Tooth & Claw Enhancement):
- Amber-orange tree resin
- Seeps from damaged bark, more after rain
- Hardens when exposed to air
- Can be consumed for temporary digging speed boosts or converted into high-yield Wildgrit (fresh > hardened)

### Crafting System

**Workshop Evolution**:
1. **Found Anvil** - Discover and roll home
2. **Basic Workshop** - Anvil + tools, basic recipes
3. **Enhanced Forge** - Add bellows and quenching pool
4. **Master Workshop** - All recipes, attracts traders

**Infusion Table**:
- Converts minerals, moonleaf, and radgum into Wildgrit (single upgrade currency)
- Spend Wildgrit here on invisible Skill Core upgrades

**Example Gear (Cosmetic/Utility)**:
- **Lantern Gem**: 1 Crystalquartz + 2 Moonleaf = Soft glow while tunneling
- **Storage Pouch**: 3 Silverflow + 4 Moonleaf = +2 carry capacity
- **Trail Markers**: 2 Copperstone + 1 Radgum = Placeable markers for navigation

---

## Environmental Design

### Forest Setting

**Vertical Layers**:
```
CANOPY: Bird nests, rare gems, aerial roots
FOREST FLOOR: Soft loam, mushroom circles, fallen logs
UNDERGROUND: Root systems, streams, ancient structures
```

### Seasonal Dynamics

- **Spring**: Soft wet soil, new growth, flooding risk
- **Summer**: Hard-packed earth, dense canopy shade
- **Autumn**: Leaf litter, nut caches, preparation time
- **Winter**: Frozen topsoil, deeper tunneling, ice paths

### Diggable Materials

```
ALWAYS DIGGABLE:
├─> Loam (1x speed)
├─> Sand (1.5x speed, requires support)
├─> Clay (0.7x speed, holds shape)
└─> Wood (0.5x speed, trees and roots)

REQUIRES UPGRADES:
├─> Stone (0.3x speed, requires Digging Mastery L2)
├─> Dense Stone (0.2x speed, requires Digging Mastery L4)
├─> Granite (0.1x speed, requires Digging Mastery L6)
└─> Crystal (0.05x speed, requires Digging Mastery L8)

SPECIAL:
├─> Hard Metal - Only during Turbo Mode (berries)
└─> Bedrock - Never diggable (world boundary)
```

---

## Core Gameplay Loop

### Daily Rhythm
```
DAY (Exploration):
├─> Roll through forest paths
├─> Discover mineral deposits
├─> Mark interesting locations
├─> Gather materials (limited carry)

EVENING (Crafting):
├─> Return to burrow workshop
├─> Start crafting queue
├─> Organize storage
└─> Plan next expedition

NIGHT (Moonleaf):
├─> Wait for leaves to unfurl
├─> Carefully harvest
├─> Avoid lunar moths
└─> Return before dawn

RAIN (Radgum):
├─> Check marked trees
├─> Collect fresh resin
├─> Quick decisions on quality
└─> Clean off sticky residue
```

### Moment-to-Moment Play

1. **Explore** with dynamic camera angles
2. **Discover** resources and challenges  
3. **Dig** through obstacles or create paths
4. **Collect** materials within carry limit
5. **Return** to burrow for crafting/storage
6. **Sleep** to save progress and reset time

---

## Special Mechanics

### Turbo Mode
- Activated by eating special berries (30-60 seconds)
- 5x dig speed, can break hard metal barriers
- Rainbow particle effects, intimidates predators
- Creates larger tunnels, uncovers hidden gems

### Skill Core Upgrades (Invisible)

Upgrades apply to stats only; no character model or obvious geometry changes. Feedback comes from feel, audio, haptics, and particles.

**Single Currency: Wildgrit**
- Earned by converting minerals, moonleaf, and radgum at the Infusion Table
- Escalating costs per level; simple two-track UI

**Digging Mastery**
- Increases dig speed and precision control
- Unlocks material thresholds: Stone (L2), Dense Stone (L4), Granite (L6), Crystal (L8)

**Rolling Mastery**
- Increases roll speed and momentum retention
- Adds hazard resistance while rolling: warm hazards (L3), brief lava contact (L5), extended lava roll (L7), heavy impact immunity (L9)
- Turbo duration/throughput scales with mastery

---

## Flow State Design

### Clear Goals
- Immediate: Dig this tunnel, collect that gem
- Short-term: Complete current episode goal
- Long-term: Build the perfect burrow network

### Immediate Feedback
- Every dig removes voxels with particles/sound
- Materials provide distinct haptic feedback
- Progress is always visible (tunnels remain)

### Challenge Balance
- Start with soft soil, progress to harder materials
- Environmental puzzles scale with abilities
- Player chooses when to tackle harder areas

---

## Unique Selling Points

1. **Universal Digging**: One button, always available, always satisfying
2. **Spherical Physics**: Roll momentum creates unique movement
3. **Living World**: Seasonal changes, weather effects, ecosystem interactions
4. **Cozy Destruction**: Destroying terrain to create homes, not for violence
5. **Contemplative Pace**: No hunger bars or urgent threats, just gentle goals

---

## Development Priorities

### MVP Features
1. Core digging and rolling mechanics
2. Basic material types (4-5)
3. Simple day/night cycle
4. One forest biome
5. Basic burrow creation
6. Camera mode switching

### Post-MVP Expansion
- Full material set
- Seasonal system
- Crafting and workshop
- Life stage progression
- Multiple biomes
- Creature interactions

---

## Success Metrics

- Average session: 20-40 minutes
- "Just one more dig" feeling
- Players create unique burrow layouts
- Screenshots of cozy homes shared
- Return rate after 1 week: 40%+

---

## Notes

This design emphasizes:
- Player expression through terrain manipulation
- Satisfying tactile feedback
- Peaceful progression without time pressure
- Discovery and collection as intrinsic rewards
- Building a home as the ultimate goal

The game answers the question: "What if you could feel the same visceral satisfaction of terrain destruction from action games, but in a peaceful, creative context?"
