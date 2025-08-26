# Game Design Exercises & Examples

## Quick-Start Exercises

### Exercise 1: The One-Mechanic Game

**Goal**: Design a complete game using only ONE core mechanic.

**Constraints**:
- Single input (one button/key/gesture)
- No power-ups or upgrades
- Difficulty comes from level design only

**Examples**:
- **Flappy Bird**: Tap to flap
- **Canabalt**: Tap to jump
- **Super Hexagon**: Rotate left/right

**Your Turn**:
1. Choose your mechanic: ________________
2. Define the challenge: ________________
3. How does difficulty scale? ________________
4. What creates replayability? ________________

---

### Exercise 2: The 60-Second Pitch

**Template**:
"[Game Name] is a [genre] game where players [core action] to [objective] while [challenge/twist]. Unlike [competitor], our game [unique element] which creates [emotional result]."

**Example**:
"Gravity Well is a physics puzzle game where players manipulate gravity fields to guide lost astronauts home while avoiding black holes. Unlike typical puzzle games, our gravity affects everything simultaneously, creating chain reactions that feel magical to discover."

**Your Turn**:
Write your 60-second pitch here:
_________________________________

---

### Exercise 3: Flow State Checklist

Evaluate your game concept against these flow criteria:

**Clear Goals**
- [ ] Player knows what to do within 5 seconds
- [ ] Objectives visible on screen
- [ ] Success conditions are obvious

**Immediate Feedback**
- [ ] Actions have response within 100ms
- [ ] Visual feedback for all inputs
- [ ] Audio confirms important actions

**Challenge-Skill Balance**
- [ ] Easy to start playing
- [ ] Difficulty ramps gradually
- [ ] Player controls difficulty (directly or indirectly)

**Deep Concentration**
- [ ] Minimal UI clutter
- [ ] No interrupting popups
- [ ] Can play for 30+ minutes uninterrupted

**Sense of Control**
- [ ] Failures feel fair
- [ ] Success feels earned
- [ ] Player agency in approach

---

## Detailed Design Examples

### Example 1: Transforming a Complex Idea into Simple Gameplay

**Complex Idea**: "A game about managing a space colony's ecosystem while dealing with political intrigue and alien threats."

**Simplification Process**:

1. **Find the Core Fun**:
   - Managing ecosystems? → Balancing connected systems
   - Political intrigue? → Making tough choices
   - Alien threats? → External pressure

2. **Choose One Focus**:
   - PRIMARY: Ecosystem balance (most unique)
   - SECONDARY: External pressure (creates urgency)
   - CUT: Political intrigue (save for sequel)

3. **Simplify Mechanics**:
   - Ecosystem = 3 resources in triangle balance
   - External pressure = Timer/wave system
   - Choices = Which system to prioritize

4. **Result**: "Balance oxygen, water, and food production in your space colony. Asteroid impacts damage systems every 60 seconds. Which will you sacrifice to keep colonists alive?"

---

### Example 2: Designing for Replayability

**Game**: Simple Racing Game

**Basic Version** (Low Replayability):
- Fixed tracks
- Best time tracking
- Linear progression

**Enhanced Version** (High Replayability):

1. **Procedural Elements**:
   - Track sections randomly combine
   - Weather affects handling
   - Traffic patterns vary

2. **Mastery Systems**:
   - Ghost races against best runs
   - Alternative racing lines
   - Risk/reward shortcuts

3. **Social Features**:
   - Daily generated tracks
   - Friend ghost challenges
   - Weekly tournaments

4. **Emergent Gameplay**:
   - Physics interactions
   - Combo system for style
   - Multiple valid strategies

---

### Example 3: Cost-Effective Prototyping

**Full Concept**: "Underwater exploration game with realistic ocean simulation"

**Greybox Prototype Version**:

**What to Include**:
- ✓ Basic swimming controls
- ✓ Pressure/oxygen mechanics
- ✓ Simple sonar ping system
- ✓ 3-4 creature types (boxes with behaviors)
- ✓ Cave system to explore

**What to Fake/Skip**:
- ✗ Realistic water rendering (use fog)
- ✗ Complex fish AI (simple patterns)
- ✗ Detailed environments (geometric shapes)
- ✗ Story elements (pure mechanics)
- ✗ Upgrade systems (locked abilities)

**Prototype Goals**:
1. Validate swimming feels good
2. Test if pressure creates tension
3. Confirm exploration is rewarding
4. Measure session length naturally

---

## Common Pitfalls & Solutions

### Pitfall 1: Feature Creep

**Symptoms**:
- "It would be cool if..."
- "Players expect..."
- "Our competitor has..."

**Solution**: The Feature Pyramid
```
Essential (MVP)
    /\
   /  \
  /Nice\
 /      \
/Future  \
----------
```

**Exercise**: List 20 features, then:
1. Choose 3 for Essential
2. Choose 5 for Nice to Have
3. Rest go to Future
4. Cut 2 from Essential

---

### Pitfall 2: Tutorial Hell

**Bad Tutorial**:
- 30 minutes of instructions
- Walls of text
- Tests instead of play

**Good Tutorial**:
- Learning by doing
- One concept at a time
- Fun from second one

**Exercise**: Design first 5 minutes
- Minute 1: ________________
- Minute 2: ________________
- Minute 3: ________________
- Minute 4: ________________
- Minute 5: ________________

---

### Pitfall 3: Complexity Addiction

**Test**: Explain your game to:
1. A gamer friend (30 seconds)
2. A non-gamer parent (1 minute)
3. A 10-year-old (2 minutes)

If any struggle, simplify.

**Simplification Techniques**:
- Combine similar systems
- Remove management layers
- Automate optimal strategies
- Cut "realistic" features

---

## Rapid Concept Testing

### The 10-10-10 Method

**10 Minutes**: Paper prototype
- Draw main screen
- Define core loop
- Walk through 1 minute of play

**10 Playtesters**: Quick feedback
- Watch them try your paper prototype
- Note confusion points
- Track excitement moments

**10 Iterations**: Refine
- Address biggest confusion
- Amplify best moment
- Simplify complex parts

---

### The Competition Analysis Matrix

| Feature        | Your Game | Competitor A | Competitor B | Your Advantage |
| -------------- | --------- | ------------ | ------------ | -------------- |
| Core Mechanic  |           |              |              |                |
| Session Length |           |              |              |                |
| Skill Ceiling  |           |              |              |                |
| Visual Style   |           |              |              |                |
| Price Point    |           |              |              |                |
| Platform       |           |              |              |                |

**Key Questions**:
1. Where are you clearly different?
2. Where are you clearly better?
3. Where are you taking risks?

---

## Weekly Design Challenges

### Week 1: One-Button Game
Design a game using only one input

### Week 2: 5-Minute Session
Design for exact 5-minute play sessions

### Week 3: No Text
Design a game that needs no text/tutorial

### Week 4: Endless Mode
Convert your concept to endless play

### Week 5: Competitive Twist
Add asynchronous multiplayer

### Week 6: Accessibility
Playable with one hand and colorblind

### Week 7: Speedrun It
Design for speedrun community

### Week 8: Mobile Port
Adapt your design for phone

---

## Final Reality Check Questions

Before moving to production, answer:

1. **Would you play this for 100 hours?**
   If no, what's missing?

2. **Can you build this in 6 months?**
   If no, what can you cut?

3. **Will someone pay $10-20 for this?**
   If no, what adds value?

4. **Does this exist already?**
   If yes, why is yours better?

5. **Can you explain the fun in one sentence?**
   If no, find the core.

Remember: A perfectly executed simple game beats a poorly executed complex one every time.
