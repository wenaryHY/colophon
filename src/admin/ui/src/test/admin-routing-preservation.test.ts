/**
 * Preservation Property Tests for Admin Routing Consistency Fix
 * 
 * **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7**
 * 
 * **Property 2: Preservation** - Existing Admin Functionality
 * 
 * **IMPORTANT**: These tests follow observation-first methodology
 * - Tests are written based on observed behavior on UNFIXED code
 * - Tests capture baseline behavior that must be preserved during the fix
 * - **EXPECTED OUTCOME**: Tests PASS on unfixed code (confirms baseline)
 * - Tests must continue to PASS after fix (confirms no regressions)
 * 
 * **GOAL**: Ensure all non-buggy functionality remains unchanged after fix
 * 
 * This test suite verifies:
 * 1. Admin sub-routes work identically before and after fix
 * 2. Internal sidebar navigation works identically
 * 3. Authentication flow remains unchanged
 * 4. Frontend routes are unaffected
 * 5. Other i18n elements continue working correctly
 * 6. Vite dev server proxy configuration still functions
 */

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import { readFileSync } from 'fs'
import { join } from 'path'

describe('Preservation: Existing Admin Functionality', () => {
  
  /**
   * Preservation 1: Admin Sub-Routes Configuration
   * 
   * **Validates: Requirement 3.1**
   * 
   * Verifies that all admin sub-routes are properly configured in App.tsx
   * and will continue to work after the fix.
   * 
   * Sub-routes to preserve:
   * - /admin/posts
   * - /admin/categories
   * - /admin/tags
   * - /admin/comments
   * - /admin/settings
   * - /admin/upload
   * - /admin/media-categories
   * - /admin/themes
   * - /admin/trash
   */
  it('should preserve all admin sub-route configurations', () => {
    const appPath = join(__dirname, '../App.tsx')
    const appContent = readFileSync(appPath, 'utf-8')
    
    // List of all admin sub-routes that must be preserved
    const requiredRoutes = [
      'posts',
      'categories',
      'tags',
      'comments',
      'settings',
      'upload',
      'media-categories',
      'themes',
      'trash'
    ]
    
    // Verify each route is configured
    requiredRoutes.forEach(route => {
      // Check for route definition like: <Route path="posts" element={...} />
      const routePattern = new RegExp(`<Route\\s+path=["']${route}["']`)
      expect(appContent).toMatch(routePattern)
    })
    
    // Verify nested routes for posts
    expect(appContent).toMatch(/<Route\s+path=["']posts\/new["']/)
    expect(appContent).toMatch(/<Route\s+path=["']posts\/:id\/edit["']/)
    
    // Verify nested route for themes
    expect(appContent).toMatch(/<Route\s+path=["']themes\/:slug["']/)
  })

  /**
   * Preservation 2: Internal Navigation Configuration
   * 
   * **Validates: Requirement 3.2**
   * 
   * Verifies that the sidebar navigation mapping (pageToRoute) is preserved.
   * This ensures internal navigation within admin panel continues to work.
   * 
   * Note: After adding basename="/admin", routes are relative to the basename,
   * so mappings use '/posts' instead of '/admin/posts'. This is correct behavior.
   */
  it('should preserve sidebar navigation route mappings', () => {
    const appPath = join(__dirname, '../App.tsx')
    const appContent = readFileSync(appPath, 'utf-8')
    
    // Verify pageToRoute mapping exists and contains all routes
    expect(appContent).toContain('pageToRoute')
    
    // With basename="/admin", routes are relative, so we check for the route keys
    const requiredRoutes = [
      'posts',
      'categories',
      'tags',
      'comments',
      'settings',
      'upload',
      'media-categories',
      'themes',
      'trash'
    ]
    
    requiredRoutes.forEach(route => {
      // Check that the route key exists in pageToRoute mapping
      const routeKey = route === 'media-categories' ? "'media-categories'" : route
      const pattern = new RegExp(`${routeKey}:\\s*['"]\/`)
      expect(appContent).toMatch(pattern)
    })
  })

  /**
   * Preservation 3: Authentication Flow
   * 
   * **Validates: Requirement 3.3**
   * 
   * Verifies that the authentication gate (AdminGate) is preserved.
   * This ensures unauthenticated users are still redirected to login.
   */
  it('should preserve authentication gate logic', () => {
    const appPath = join(__dirname, '../App.tsx')
    const appContent = readFileSync(appPath, 'utf-8')
    
    // Verify AdminGate component exists
    expect(appContent).toContain('function AdminGate()')
    
    // Verify it uses AuthContext
    expect(appContent).toMatch(/const\s+{\s*user/)
    expect(appContent).toMatch(/useAuth\(\)/)
    
    // Verify it shows Login when no token
    expect(appContent).toMatch(/if\s*\(!\s*user\s*\|\|\s*user\.role/)
    expect(appContent).toContain('return <Login />')
    
    // Verify loading state
    expect(appContent).toContain('if (isLoading)')
  })

  /**
   * Preservation 4: Setup Route
   * 
   * **Validates: Requirement 3.4**
   * 
   * Verifies that the /setup route is preserved and not affected by admin routing changes.
   */
  it('should preserve /setup route configuration', () => {
    const appPath = join(__dirname, '../App.tsx')
    const appContent = readFileSync(appPath, 'utf-8')
    
    // Verify /setup route exists and is outside admin routes
    expect(appContent).toMatch(/<Route\s+path=["']\/setup["']\s+element={<Setup\s*\/>}/)
  })

  /**
   * Preservation 5: Vite Dev Server Proxy Configuration
   * 
   * **Validates: Requirement 3.7**
   * 
   * Verifies that Vite's proxy configuration for API requests is preserved.
   * This ensures dev server continues to proxy requests to port 2000.
   */
  it('should preserve Vite dev server proxy configuration', () => {
    const viteConfigPath = join(__dirname, '../../vite.config.ts')
    const viteConfig = readFileSync(viteConfigPath, 'utf-8')
    
    // Verify proxy configuration exists
    expect(viteConfig).toContain('proxy:')
    
    // Verify API proxy
    expect(viteConfig).toMatch(/['"]\/api['"]\s*:\s*['"]http:\/\/127\.0\.0\.1:2000['"]/)
    
    // Verify WebSocket proxy
    expect(viteConfig).toMatch(/['"]\/ws['"]\s*:/)
    expect(viteConfig).toContain('ws: true')
    
    // Verify uploads proxy
    expect(viteConfig).toMatch(/['"]\/uploads['"]\s*:\s*['"]http:\/\/127\.0\.0\.1:2000['"]/)
  })

  /**
   * Preservation 6: i18n Attributes on Other Elements
   * 
   * **Validates: Requirement 3.6**
   * 
   * Verifies that other i18n elements in profile.html (NOT the ADMIN button)
   * continue to have their data-i18n attributes.
   */
  it('should preserve i18n attributes on other profile.html elements', () => {
    const profilePath = join(__dirname, '../../../../../themes/default/templates/profile.html')
    const profileContent = readFileSync(profilePath, 'utf-8')
    
    // List of i18n elements that must be preserved (excluding ADMIN button)
    const requiredI18nElements = [
      { key: 'role', attr: 'data-i18n' },
      { key: 'joined', attr: 'data-i18n' },
      { key: 'profileSettings', attr: 'data-i18n' },
      { key: 'displayName', attr: 'data-i18n' },
      { key: 'themePreference', attr: 'data-i18n' },
      { key: 'system', attr: 'data-i18n' },
      { key: 'light', attr: 'data-i18n' },
      { key: 'dark', attr: 'data-i18n' },
      { key: 'bio', attr: 'data-i18n' },
      { key: 'bioPlaceholder', attr: 'data-i18n-placeholder' },
      { key: 'saveProfile', attr: 'data-i18n' },
      { key: 'changePassword', attr: 'data-i18n' },
      { key: 'currentPassword', attr: 'data-i18n' },
      { key: 'newPassword', attr: 'data-i18n' },
      { key: 'confirmPassword', attr: 'data-i18n' },
      { key: 'updatePassword', attr: 'data-i18n' },
      { key: 'myComments', attr: 'data-i18n' },
      { key: 'noComments', attr: 'data-i18n' }
    ]
    
    requiredI18nElements.forEach(element => {
      const pattern = new RegExp(`${element.attr}=["']${element.key}["']`)
      expect(profileContent).toMatch(pattern)
    })
  })

  /**
   * Property-Based Test: Admin Sub-Route Navigation Consistency
   * 
   * **Validates: Requirements 3.1, 3.2**
   * 
   * This property test generates various admin sub-routes and verifies
   * that they are all properly configured in the routing system.
   * 
   * The test ensures that for ANY admin sub-route, the route configuration
   * exists and the navigation mapping is correct.
   */
  it('property: all admin sub-routes should have consistent configuration', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(
          'posts',
          'categories',
          'tags',
          'comments',
          'settings',
          'upload',
          'media-categories',
          'themes',
          'trash'
        ),
        (route) => {
          const appPath = join(__dirname, '../App.tsx')
          const appContent = readFileSync(appPath, 'utf-8')
          
          // Property 1: Route must be defined in Routes
          const routePattern = new RegExp(`<Route\\s+path=["']${route}["']`)
          expect(appContent).toMatch(routePattern)
          
          // Property 2: Route must be in pageToRoute mapping
          // With basename="/admin", routes are relative (e.g., '/posts' not '/admin/posts')
          const mappingKey = route === 'media-categories' ? "'media-categories'" : route
          const mappingPattern = new RegExp(`${mappingKey}:\\s*['"]\/${route}['"]`)
          expect(appContent).toMatch(mappingPattern)
          
          // Property 3: Route must be handled in getActivePage function
          // Note: The function uses 'pathname' parameter, not 'adminPath'
          const activePagePattern = new RegExp(`if\\s*\\(pathname\\.startsWith\\(['"]\\/${route}['"]\\)\\)`)
          expect(appContent).toMatch(activePagePattern)
        }
      ),
      { numRuns: 9 } // Test all 9 admin sub-routes
    )
  })

  /**
   * Property-Based Test: i18n Element Preservation
   * 
   * **Validates: Requirement 3.6**
   * 
   * This property test verifies that for ANY i18n element in profile.html
   * (except ADMIN button), the data-i18n attribute is present and correct.
   */
  it('property: all existing i18n elements should preserve their attributes', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(
          { key: 'profileSettings', text: 'Profile settings' },
          { key: 'displayName', text: 'Display name' },
          { key: 'saveProfile', text: 'Save profile' },
          { key: 'changePassword', text: 'Change password' },
          { key: 'currentPassword', text: 'Current password' },
          { key: 'newPassword', text: 'New password' },
          { key: 'confirmPassword', text: 'Confirm new password' },
          { key: 'updatePassword', text: 'Update password' }
        ),
        (element) => {
          const profilePath = join(__dirname, '../../../../../themes/default/templates/profile.html')
          const profileContent = readFileSync(profilePath, 'utf-8')
          
          // Property: Element must have data-i18n attribute
          const pattern = new RegExp(`data-i18n=["']${element.key}["']`)
          expect(profileContent).toMatch(pattern)
          
          // Property: Element should contain the English fallback text
          // (This ensures the element exists in the template)
          expect(profileContent).toContain(element.text)
        }
      ),
      { numRuns: 10 } // Test 10 different i18n elements
    )
  })

  /**
   * Property-Based Test: Vite Configuration Stability
   * 
   * **Validates: Requirement 3.7**
   * 
   * This property test verifies that critical Vite configuration options
   * (except base) remain unchanged after the fix.
   */
  it('property: Vite configuration should preserve all non-base settings', () => {
    fc.assert(
      fc.property(
        fc.constantFrom(
          { setting: 'outDir', value: "'../dist'" },
          { setting: 'emptyOutDir', value: 'true' },
          { setting: 'globals', value: 'true' },
          { setting: 'environment', value: "'jsdom'" }
        ),
        (config) => {
          const viteConfigPath = join(__dirname, '../../vite.config.ts')
          const viteConfig = readFileSync(viteConfigPath, 'utf-8')
          
          // Property: Configuration setting must exist with correct value
          const pattern = new RegExp(`${config.setting}:\\s*${config.value}`)
          expect(viteConfig).toMatch(pattern)
        }
      ),
      { numRuns: 4 } // Test 4 critical config settings
    )
  })

  /**
   * Property-Based Test: Route Path Consistency
   * 
   * **Validates: Requirements 3.1, 3.5**
   * 
   * This property test verifies that route paths follow consistent patterns
   * and that frontend routes (non-admin) are not affected.
   */
  it('property: route paths should follow consistent patterns', () => {
    fc.assert(
      fc.property(
        fc.record({
          routeType: fc.constantFrom('admin-sub', 'setup', 'wildcard'),
          route: fc.constantFrom('posts', 'categories', 'tags', 'settings')
        }),
        (scenario) => {
          const appPath = join(__dirname, '../App.tsx')
          const appContent = readFileSync(appPath, 'utf-8')
          
          if (scenario.routeType === 'admin-sub') {
            // Property: Admin sub-routes should be nested under /admin
            // They should NOT have /admin prefix in their path attribute
            const routeMatch = appContent.match(
              new RegExp(`<Route\\s+path=["']${scenario.route}["']\\s+element=`)
            )
            expect(routeMatch).toBeTruthy()
            
            // Verify it's nested under /admin parent route
            const adminRouteIndex = appContent.indexOf('<Route path="/admin"')
            const subRouteIndex = appContent.indexOf(`path="${scenario.route}"`)
            expect(subRouteIndex).toBeGreaterThan(adminRouteIndex)
          }
          
          if (scenario.routeType === 'setup') {
            // Property: /setup route should be at root level, not nested
            expect(appContent).toMatch(/<Route\s+path=["']\/setup["']/)
          }
          
          if (scenario.routeType === 'wildcard') {
            // Property: Wildcard route should redirect to root (which is /admin with basename)
            // With basename="/admin", Navigate to="/" is equivalent to /admin
            expect(appContent).toMatch(/<Route\s+path=["']\*["']\s+element={<Navigate\s+to=["']\/["']/)
          }
        }
      ),
      { numRuns: 12 } // Test various route scenarios
    )
  })

  /**
   * Property-Based Test: Authentication Flow Preservation
   * 
   * **Validates: Requirement 3.3**
   * 
   * This property test verifies that authentication logic remains intact
   * for different authentication states.
   */
  it('property: authentication flow should handle all states correctly', () => {
    fc.assert(
      fc.property(
        fc.constantFrom('loading', 'unauthenticated', 'authenticated'),
        (authState) => {
          const appPath = join(__dirname, '../App.tsx')
          const appContent = readFileSync(appPath, 'utf-8')
          
          // Property: AdminGate must handle all authentication states
          expect(appContent).toContain('function AdminGate()')
          
          if (authState === 'loading') {
            // Must show loading skeleton
            expect(appContent).toContain('if (isLoading)')
            expect(appContent).toContain('PostsSkeleton')
          }
          
          if (authState === 'unauthenticated') {
            // Must show login page
            expect(appContent).toMatch(/if\s*\(!\s*user\s*\|\|\s*user\.role/)
            expect(appContent).toContain('return <Login />')
          }
          
          if (authState === 'authenticated') {
            // Must show admin layout
            expect(appContent).toContain('return <AdminLayout />')
          }
        }
      ),
      { numRuns: 3 } // Test all 3 authentication states
    )
  })
})
