/* ========================================================================
 * Project: pharos
 * Component: Marketing Site - MDX Plugin
 * File: remarkStripPrologue.mjs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * A custom Remark plugin to identify and strip out the standardized 
 * file prologue block so it does not render on the public documentation pages.
 * * Traceability:
 * Related to user feedback on Issue #36 regarding public rendering.
 * ======================================================================== */

export default function remarkStripPrologue() {
  return (tree) => {
    let startIndex = -1;
    let endIndex = -1;

    // Iterate through the top-level nodes of the AST
    for (let i = 0; i < tree.children.length; i++) {
      const node = tree.children[i];
      // Stringify the node to easily search its contents regardless of how 
      // the Markdown parser (MDX) interpreted the prologue (e.g., as lists or paragraphs)
      const nodeStr = JSON.stringify(node);
      
      // Find the start of the prologue
      if (startIndex === -1 && nodeStr.includes('/* ========')) {
        startIndex = i;
      }
      
      // Find the end of the prologue
      if (startIndex !== -1 && endIndex === -1 && nodeStr.includes('======== */')) {
        endIndex = i;
        break;
      }
    }

    // If both start and end are found, remove the entire block of nodes
    if (startIndex !== -1 && endIndex !== -1) {
      tree.children.splice(startIndex, endIndex - startIndex + 1);
    }
  };
}
