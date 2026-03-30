/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/middleware.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Global middleware for the Web Console. Enforces authentication by 
 * verifying JWT sessions before allowing access to protected routes.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */
import { defineMiddleware } from 'astro:middleware';
import { getSession } from './features/auth/session-logic';
import { AUTH_COOKIE_NAME } from './features/auth/auth-config';

export const onRequest = defineMiddleware(async (context, next) => {
    const { url, cookies, redirect } = context;

    // Handle documentation redirect
    if (url.pathname === '/docs' || url.pathname === '/docs/') {
        return redirect('https://iamrichardd.com/pharos/docs/');
    }

    // Verify session
    const sessionCookie = cookies.get(AUTH_COOKIE_NAME)?.value;
    let session = await getSession(sessionCookie);
    
    // Support PHAROS_SKIP_AUTH for E2E testing / Sandbox dev
    if (!session && process.env.PHAROS_SKIP_AUTH === 'true') {
        session = { userId: 'admin', roles: ['admin'], sub: 'admin' };
    }

    // Attach session to locals for use in pages/components
    if (session) {
        context.locals.session = session;
    }

    // Enforce password change if flagged
    const isMcpRoute = url.pathname.startsWith('/mcp');
    if (session?.mustChangePassword && url.pathname !== '/change-password' && !url.pathname.startsWith('/_actions') && !isMcpRoute) {
        return redirect('/change-password');
    }

    // Public routes that don't require authentication
    // Note: /_actions must be allowed so that the login action can be called
    // Note: /mcp is public but individual methods enforce auth in mcp.ts
    const publicRoutes = ['/login', '/_actions', '/mdb', '/add-node', '/mcp'];
    const isPublicRoute = (publicRoutes.some(route => url.pathname.startsWith(route)) || url.pathname === '/') && !url.pathname.startsWith('/mdb/add');

    if (isPublicRoute) {
        return next();
    }

    if (!session) {
        // Redirect to login if unauthenticated on a protected route
        return redirect('/login');
    }

    return next();
});
