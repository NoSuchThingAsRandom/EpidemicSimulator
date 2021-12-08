# Approach

* Build graph in rust
* Generate image using sfdp
  (Can use -Goverlap=scale instead?)
  - For png -> `sfdp -x -Goverlap=prism -Tpng input.dot > output.png`
  - For svg -> `sfdp -x -Goverlap=prism -Tsvg input.dot > output.svg`
* Can only render in GIMP

## Times

* 200K nodes many edges -> 30minutes for rendering (Rendering every Citizen)
* 10K nodes, 15K edges much faster (This was only rendering Buildings, with massive households and workplace sizes)

Is utterly useless visualisations as one massive cluster


