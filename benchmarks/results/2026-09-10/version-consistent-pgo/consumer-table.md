| Project / mode | Oriole normal ms | Oriole PGO ms | Expat normal ms | Expat PGO ms | Normal O/E | PGO O/E |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| vulkan / elementtree | 144.503608 | 116.686502 | 85.410315 | 81.739830 | 1.666522 | 1.447729 |
| vulkan / pyexpat-events | 240.061146 | 212.211139 | 161.887806 | 154.086726 | 1.482620 | 1.356858 |
| wayland / elementtree | 2.675668 | 2.194519 | 1.756511 | 1.619711 | 1.518717 | 1.364173 |
| wayland / pyexpat-events | 3.369515 | 2.921752 | 3.165848 | 3.008224 | 1.065492 | 0.973596 |
| maven / elementtree | 2.250975 | 1.742775 | 0.929536 | 0.863078 | 2.421549 | 2.019257 |
| maven / pyexpat-events | 2.975522 | 2.421002 | 1.676376 | 1.569902 | 1.774973 | 1.540730 |
| batik / elementtree | 0.279483 | 0.239413 | 0.211700 | 0.203255 | 1.320264 | 1.175166 |
| batik / pyexpat-events | 0.306292 | 0.263703 | 0.248769 | 0.239358 | 1.222620 | 1.101731 |
| gtk / elementtree | 0.919753 | 0.747241 | 0.431557 | 0.401038 | 2.134310 | 1.863267 |
| gtk / pyexpat-events | 1.222538 | 1.030714 | 0.735543 | 0.695288 | 1.659748 | 1.486765 |
| docbook / elementtree | 0.743910 | 0.601062 | 0.392794 | 0.391205 | 1.893892 | 1.533683 |
| docbook / pyexpat-events | 0.913952 | 0.768600 | 0.578225 | 0.553588 | 1.589974 | 1.388397 |

Medians shown are medians of the seven per-cohort worker medians. Ratios are medians of the seven directly paired ratios, so rounded displayed times should not be divided to reconstruct them. All actual-consumer and fair-PGO ratios retain both chunk widths in consumer-reconstruction.json.
