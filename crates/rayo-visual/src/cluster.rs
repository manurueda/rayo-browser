use serde::Serialize;

/// A contiguous region of changed pixels.
#[derive(Debug, Clone, Serialize)]
pub struct ChangedRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Local diff density within this region (0.0-1.0).
    pub diff_ratio: f64,
}

/// Cluster nearby diff pixels into regions using the block grid from pixel diff.
///
/// `block_diffs`: 2D grid of diff pixel counts per block.
/// `block_size`: pixel size of each block (e.g., 32).
/// `image_width`, `image_height`: full image dimensions for clamping.
pub fn cluster_regions(
    block_diffs: &[Vec<u32>],
    block_rows: usize,
    block_cols: usize,
    block_size: u32,
    image_width: u32,
    image_height: u32,
) -> Vec<ChangedRegion> {
    if block_rows == 0 || block_cols == 0 {
        return Vec::new();
    }

    // Mark blocks that have any diffs
    let mut visited = vec![vec![false; block_cols]; block_rows];
    let mut regions = Vec::new();

    for row in 0..block_rows {
        for col in 0..block_cols {
            if block_diffs[row][col] > 0 && !visited[row][col] {
                // Flood-fill to find connected component
                let mut min_col = col;
                let mut max_col = col;
                let mut min_row = row;
                let mut max_row = row;
                let mut total_diff_pixels = 0u32;

                let mut stack = vec![(row, col)];
                while let Some((r, c)) = stack.pop() {
                    if r >= block_rows || c >= block_cols || visited[r][c] || block_diffs[r][c] == 0
                    {
                        continue;
                    }
                    visited[r][c] = true;
                    total_diff_pixels += block_diffs[r][c];

                    min_col = min_col.min(c);
                    max_col = max_col.max(c);
                    min_row = min_row.min(r);
                    max_row = max_row.max(r);

                    // 4-connected neighbors
                    if r > 0 {
                        stack.push((r - 1, c));
                    }
                    if r + 1 < block_rows {
                        stack.push((r + 1, c));
                    }
                    if c > 0 {
                        stack.push((r, c - 1));
                    }
                    if c + 1 < block_cols {
                        stack.push((r, c + 1));
                    }
                }

                let x = (min_col as u32) * block_size;
                let y = (min_row as u32) * block_size;
                let width = ((max_col - min_col + 1) as u32 * block_size).min(image_width - x);
                let height = ((max_row - min_row + 1) as u32 * block_size).min(image_height - y);
                let region_pixels = width * height;
                let diff_ratio = if region_pixels > 0 {
                    total_diff_pixels as f64 / region_pixels as f64
                } else {
                    0.0
                };

                regions.push(ChangedRegion {
                    x,
                    y,
                    width,
                    height,
                    diff_ratio,
                });
            }
        }
    }

    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_diffs_produces_no_regions() {
        let grid = vec![vec![0u32; 4]; 4];
        let regions = cluster_regions(&grid, 4, 4, 32, 128, 128);
        assert!(regions.is_empty());
    }

    #[test]
    fn single_block_diff_produces_one_region() {
        let mut grid = vec![vec![0u32; 4]; 4];
        grid[1][2] = 50;
        let regions = cluster_regions(&grid, 4, 4, 32, 128, 128);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].x, 64);
        assert_eq!(regions[0].y, 32);
        assert_eq!(regions[0].width, 32);
        assert_eq!(regions[0].height, 32);
    }

    #[test]
    fn adjacent_blocks_merge_into_one_region() {
        let mut grid = vec![vec![0u32; 4]; 4];
        grid[1][1] = 10;
        grid[1][2] = 20;
        grid[2][1] = 30;
        let regions = cluster_regions(&grid, 4, 4, 32, 128, 128);
        assert_eq!(regions.len(), 1);
        // Should cover blocks (1,1) to (2,2)
        assert_eq!(regions[0].x, 32);
        assert_eq!(regions[0].y, 32);
        assert_eq!(regions[0].width, 64);
        assert_eq!(regions[0].height, 64);
    }

    #[test]
    fn disjoint_blocks_produce_separate_regions() {
        let mut grid = vec![vec![0u32; 8]; 8];
        grid[0][0] = 10;
        grid[7][7] = 20;
        let regions = cluster_regions(&grid, 8, 8, 32, 256, 256);
        assert_eq!(regions.len(), 2);
    }
}
