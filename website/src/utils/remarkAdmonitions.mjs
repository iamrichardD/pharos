/* ========================================================================
 * Project: pharos
 * Component: Marketing Site - MDX Plugin
 * File: remarkAdmonitions.mjs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Transforms `:::tip[Title]` / `:::caution` / `:::warning` / `:::note` /
 * `:::danger` container directives (parsed by remark-directive) into styled
 * HTML. Without this, the directive syntax used throughout the docs renders
 * as literal, unstyled text (e.g. ":::tip[One-Liner Installation]") since
 * remark-directive only parses the syntax into an AST node — it doesn't
 * decide how that node becomes HTML.
 * * Traceability:
 * Fixes pre-existing broken rendering of :::tip blocks across the docs site.
 * ======================================================================== */

import { visit } from 'unist-util-visit';

const KNOWN_TYPES = new Set(['tip', 'note', 'caution', 'warning', 'danger']);

export default function remarkAdmonitions() {
  return (tree) => {
    visit(tree, (node) => node.type === 'containerDirective', (node) => {
      if (!KNOWN_TYPES.has(node.name)) {
        return;
      }

      node.data ??= {};
      node.data.hName = 'div';
      node.data.hProperties = { className: ['admonition', `admonition-${node.name}`] };

      const labelIndex = node.children.findIndex(
        (child) => child.data && child.data.directiveLabel
      );

      if (labelIndex !== -1) {
        const label = node.children[labelIndex];
        label.data.hName = 'p';
        label.data.hProperties = { className: ['admonition-title'] };
      } else {
        const defaultTitle = node.name.charAt(0).toUpperCase() + node.name.slice(1);
        node.children.unshift({
          type: 'paragraph',
          data: { hName: 'p', hProperties: { className: ['admonition-title'] } },
          children: [{ type: 'text', value: defaultTitle }],
        });
      }
    });
  };
}
