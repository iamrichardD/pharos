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

    // Public routes that don't require authentication
    const publicRoutes = ['/login', '/api/actions/login'];
    const isPublicRoute = publicRoutes.some(route => url.pathname.startsWith(route));

    if (isPublicRoute) {
        return next();
    }

    // Verify session
    const session = await getSession(cookies.get(AUTH_COOKIE_NAME)?.value);

    if (!session) {
        // Redirect to login if unauthenticated on a protected route
        return redirect('/login');
    }

    // Attach session to locals for use in pages/components
    context.locals.session = session;

    return next();
});
