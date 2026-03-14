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
    const session = await getSession(cookies.get(AUTH_COOKIE_NAME)?.value);
    
    // Attach session to locals for use in pages/components
    if (session) {
        context.locals.session = session;
    }

    // Enforce password change if flagged
    if (session?.mustChangePassword && url.pathname !== '/change-password' && !url.pathname.startsWith('/_actions')) {
        return redirect('/change-password');
    }

    // Public routes that don't require authentication
    // Note: /_actions must be allowed so that the login action can be called
    const publicRoutes = ['/login', '/_actions', '/mdb', '/add-node'];
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
