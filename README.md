# Wolfsmühle - Eifeler Brettspiel

A board game from the German
[Eifel](https://en.wikipedia.org/wiki/Eifel)
region.

![image](doc/wolfsmuehle.png)

## Game rules

### Moving rules

- Players can move by one grid position on every turn.
- Wolves are free to move in any direction on the grid.
- Sheep can only move vertically upwards towards the barn, or horizontally.
  Never downwards or diagonally.
- Moving turns are alternating between wolves and sheep.

### Sheep win, if

- Nine sheep have occupied the barn at the top.
- Or if both wolves are surrounded by sheep such as that the wolves can't move anymore.

### Wolves win, if

- Less than nine sheep are left on the board.

### Capturing sheep

- A wolf can capture sheep by jumping over one, if a sheep is directly adjacent to the wolf and the jump destination position is not occupied.
  A capture move distance is exactly two grid positions.
- Wolves cannot capture across 90 degree corners.
  But capturing across 135 degree corners is possible.

