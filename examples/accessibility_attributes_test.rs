//! Example to test macOS accessibility attributes and see what window identification
//! information can be obtained from the currently focused window.

use accessibility_sys::*;
use core_foundation::base::{CFType, TCFType};
use core_foundation::string::CFString;
use objc2::{class, msg_send, sel, sel_impl};
use std::ffi;
use std::ptr;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing macOS Accessibility Attributes");
    println!("======================================");
    println!();

    if !unsafe { AXIsProcessTrusted() } {
        eprintln!("Error: Accessibility permissions required. Please enable accessibility access in System Settings > Privacy & Security > Accessibility.");
        eprintln!(
            "Note: You may need to grant permission to your terminal app (Terminal, iTerm, etc.)"
        );
        return Ok(());
    }

    println!("Accessibility permissions verified");
    println!();

    // Get the currently focused window
    let focused_window = get_focused_window()?;
    println!("Successfully obtained focused window element");
    println!();

    // Test all accessibility attributes
    test_accessibility_attributes(focused_window);

    Ok(())
}

fn get_focused_window() -> Result<AXUIElementRef, Box<dyn std::error::Error>> {
    use objc2::{class, msg_send, sel, sel_impl};

    unsafe {
        // Get the frontmost application using NSWorkspace (more reliable approach)
        let workspace_class = class!(NSWorkspace);
        let workspace: *mut objc2::runtime::Object = msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut objc2::runtime::Object = msg_send![workspace, frontmostApplication];

        if frontmost_app.is_null() {
            return Err("No frontmost application found".into());
        }

        let pid: i32 = msg_send![frontmost_app, processIdentifier];
        println!("Frontmost app PID: {}", pid);

        // Create application element
        let app_element = accessibility_sys::AXUIElementCreateApplication(pid);

        // Get focused window from application
        let mut focused_window: *mut ffi::c_void = ptr::null_mut();
        let focused_window_attr = CFString::from_static_string(kAXFocusedWindowAttribute);
        let result = AXUIElementCopyAttributeValue(
            app_element,
            focused_window_attr.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut ffi::c_void>(&mut focused_window)
                .cast::<*const ffi::c_void>(),
        );

        if result != 0 || focused_window.is_null() {
            return Err("Failed to get focused window".into());
        }

        Ok(focused_window as AXUIElementRef)
    }
}

fn test_accessibility_attributes(window: AXUIElementRef) {
    println!("Testing accessibility attributes for focused window:");
    println!("---------------------------------------------------");

    // List of attributes to test (from the provided list)
    let attributes = vec![
        ("AXRole", kAXRoleAttribute),
        ("AXSubrole", kAXSubroleAttribute),
        ("AXRoleDescription", kAXRoleDescriptionAttribute),
        ("AXHelp", kAXHelpAttribute),
        ("AXTitle", kAXTitleAttribute),
        ("AXValue", kAXValueAttribute),
        ("AXValueDescription", kAXValueDescriptionAttribute),
        ("AXMinValue", kAXMinValueAttribute),
        ("AXMaxValue", kAXMaxValueAttribute),
        ("AXValueIncrement", kAXValueIncrementAttribute),
        ("AXAllowedValues", kAXAllowedValuesAttribute),
        ("AXPlaceholderValue", kAXPlaceholderValueAttribute),
        ("AXEnabled", kAXEnabledAttribute),
        ("AXElementBusy", kAXElementBusyAttribute),
        ("AXFocused", kAXFocusedAttribute),
        ("AXParent", kAXParentAttribute),
        ("AXChildren", kAXChildrenAttribute),
        ("AXSelectedChildren", kAXSelectedChildrenAttribute),
        ("AXVisibleChildren", kAXVisibleChildrenAttribute),
        ("AXWindow", kAXWindowAttribute),
        ("AXTopLevelUIElement", kAXTopLevelUIElementAttribute),
        ("AXPosition", kAXPositionAttribute),
        ("AXSize", kAXSizeAttribute),
        ("AXOrientation", kAXOrientationAttribute),
        ("AXDescription", kAXDescriptionAttribute),
        ("AXSelectedText", kAXSelectedTextAttribute),
        ("AXSelectedTextRange", kAXSelectedTextRangeAttribute),
        ("AXSelectedTextRanges", kAXSelectedTextRangesAttribute),
        ("AXVisibleCharacterRange", kAXVisibleCharacterRangeAttribute),
        ("AXNumberOfCharacters", kAXNumberOfCharactersAttribute),
        ("AXSharedTextUIElements", kAXSharedTextUIElementsAttribute),
        ("AXSharedCharacterRange", kAXSharedCharacterRangeAttribute),
        ("AXSharedFocusElements", kAXSharedFocusElementsAttribute),
        (
            "AXInsertionPointLineNumber",
            kAXInsertionPointLineNumberAttribute,
        ),
        ("AXMain", kAXMainAttribute),
        ("AXMinimized", kAXMinimizedAttribute),
        ("AXCloseButton", kAXCloseButtonAttribute),
        ("AXZoomButton", kAXZoomButtonAttribute),
        ("AXMinimizeButton", kAXMinimizeButtonAttribute),
        ("AXToolbarButton", kAXToolbarButtonAttribute),
        ("AXFullScreenButton", kAXFullScreenButtonAttribute),
        ("AXProxy", kAXProxyAttribute),
        ("AXGrowArea", kAXGrowAreaAttribute),
        ("AXModal", kAXModalAttribute),
        ("AXDefaultButton", kAXDefaultButtonAttribute),
        ("AXCancelButton", kAXCancelButtonAttribute),
        ("AXMenuItemCmdChar", kAXMenuItemCmdCharAttribute),
        ("AXMenuItemCmdVirtualKey", kAXMenuItemCmdVirtualKeyAttribute),
        ("AXMenuItemCmdGlyph", kAXMenuItemCmdGlyphAttribute),
        ("AXMenuItemCmdModifiers", kAXMenuItemCmdModifiersAttribute),
        ("AXMenuItemMarkChar", kAXMenuItemMarkCharAttribute),
        (
            "AXMenuItemPrimaryUIElement",
            kAXMenuItemPrimaryUIElementAttribute,
        ),
        ("AXMenuBar", kAXMenuBarAttribute),
        ("AXWindows", kAXWindowsAttribute),
        ("AXFrontmost", kAXFrontmostAttribute),
        ("AXHidden", kAXHiddenAttribute),
        ("AXMainWindow", kAXMainWindowAttribute),
        ("AXFocusedWindow", kAXFocusedWindowAttribute),
        ("AXFocusedUIElement", kAXFocusedUIElementAttribute),
        ("AXExtrasMenuBar", kAXExtrasMenuBarAttribute),
        ("AXHeader", kAXHeaderAttribute),
        ("AXEdited", kAXEditedAttribute),
        ("AXValueWraps", kAXValueWrapsAttribute),
        ("AXTabs", kAXTabsAttribute),
        ("AXTitleUIElement", kAXTitleUIElementAttribute),
        ("AXHorizontalScrollBar", kAXHorizontalScrollBarAttribute),
        ("AXVerticalScrollBar", kAXVerticalScrollBarAttribute),
        ("AXOverflowButton", kAXOverflowButtonAttribute),
        ("AXFilename", kAXFilenameAttribute),
        ("AXExpanded", kAXExpandedAttribute),
        ("AXSelected", kAXSelectedAttribute),
        ("AXSplitters", kAXSplittersAttribute),
        ("AXNextContents", kAXNextContentsAttribute),
        ("AXDocument", kAXDocumentAttribute),
        ("AXDecrementButton", kAXDecrementButtonAttribute),
        ("AXIncrementButton", kAXIncrementButtonAttribute),
        ("AXPreviousContents", kAXPreviousContentsAttribute),
        ("AXContents", kAXContentsAttribute),
        ("AXIncrementor", kAXIncrementorAttribute),
        ("AXHourField", kAXHourFieldAttribute),
        ("AXMinuteField", kAXMinuteFieldAttribute),
        ("AXSecondField", kAXSecondFieldAttribute),
        ("AXAMPMField", kAXAMPMFieldAttribute),
        ("AXDayField", kAXDayFieldAttribute),
        ("AXMonthField", kAXMonthFieldAttribute),
        ("AXYearField", kAXYearFieldAttribute),
        ("AXColumnTitles", kAXColumnTitlesAttribute),
        ("AXURL", kAXURLAttribute),
        ("AXLabelUIElements", kAXLabelUIElementsAttribute),
        ("AXLabelValue", kAXLabelValueAttribute),
        ("AXShownMenuUIElement", kAXShownMenuUIElementAttribute),
        (
            "AXServesAsTitleForUIElements",
            kAXServesAsTitleForUIElementsAttribute,
        ),
        ("AXLinkedUIElements", kAXLinkedUIElementsAttribute),
        ("AXRows", kAXRowsAttribute),
        ("AXVisibleRows", kAXVisibleRowsAttribute),
        ("AXSelectedRows", kAXSelectedRowsAttribute),
        ("AXColumns", kAXColumnsAttribute),
        ("AXVisibleColumns", kAXVisibleColumnsAttribute),
        ("AXSelectedColumns", kAXSelectedColumnsAttribute),
        ("AXSortDirection", kAXSortDirectionAttribute),
        ("AXIndex", kAXIndexAttribute),
        ("AXDisclosing", kAXDisclosingAttribute),
        ("AXDisclosedRows", kAXDisclosedRowsAttribute),
        ("AXDisclosedByRow", kAXDisclosedByRowAttribute),
        ("AXDisclosureLevel", kAXDisclosureLevelAttribute),
        ("AXMatteHole", kAXMatteHoleAttribute),
        ("AXMatteContentUIElement", kAXMatteContentUIElementAttribute),
        ("AXMarkerUIElements", kAXMarkerUIElementsAttribute),
        ("AXUnits", kAXUnitsAttribute),
        ("AXUnitDescription", kAXUnitDescriptionAttribute),
        ("AXMarkerType", kAXMarkerTypeAttribute),
        ("AXMarkerTypeDescription", kAXMarkerTypeDescriptionAttribute),
        ("AXIsApplicationRunning", kAXIsApplicationRunningAttribute),
        ("AXSearchButton", kAXSearchButtonAttribute),
        ("AXClearButton", kAXClearButtonAttribute),
        ("AXFocusedApplication", kAXFocusedApplicationAttribute),
        ("AXRowCount", kAXRowCountAttribute),
        ("AXColumnCount", kAXColumnCountAttribute),
        ("AXOrderedByRow", kAXOrderedByRowAttribute),
        ("AXWarningValue", kAXWarningValueAttribute),
        ("AXCriticalValue", kAXCriticalValueAttribute),
        ("AXSelectedCells", kAXSelectedCellsAttribute),
        ("AXVisibleCells", kAXVisibleCellsAttribute),
        ("AXRowHeaderUIElements", kAXRowHeaderUIElementsAttribute),
        (
            "AXColumnHeaderUIElements",
            kAXColumnHeaderUIElementsAttribute,
        ),
        ("AXRowIndexRange", kAXRowIndexRangeAttribute),
        ("AXColumnIndexRange", kAXColumnIndexRangeAttribute),
        ("AXHorizontalUnits", kAXHorizontalUnitsAttribute),
        ("AXVerticalUnits", kAXVerticalUnitsAttribute),
        (
            "AXHorizontalUnitDescription",
            kAXHorizontalUnitDescriptionAttribute,
        ),
        (
            "AXVerticalUnitDescription",
            kAXVerticalUnitDescriptionAttribute,
        ),
        ("AXHandles", kAXHandlesAttribute),
        ("AXText", kAXTextAttribute),
        ("AXVisibleText", kAXVisibleTextAttribute),
        ("AXIsEditable", kAXIsEditableAttribute),
        ("AXIdentifier", kAXIdentifierAttribute),
        ("AXAlternateUIVisible", kAXAlternateUIVisibleAttribute),
    ];

    let mut available_count = 0;
    let total_count = attributes.len();

    for (name, attr) in attributes {
        match get_attribute_value(window, attr) {
            Ok(Some(value)) => {
                println!("✓ {}: {}", name, value);
                available_count += 1;
            }
            Ok(None) => {
                println!("- {}: <no value>", name);
            }
            Err(_) => {
                println!("✗ {}: <not available>", name);
            }
        }
    }

    println!();
    println!("Summary:");
    println!("  Total attributes tested: {}", total_count);
    println!("  Attributes with values: {}", available_count);
    println!(
        "  Attributes not available: {}",
        total_count - available_count
    );
}

fn get_attribute_value(
    window: AXUIElementRef,
    attribute: &'static str,
) -> Result<Option<String>, ()> {
    unsafe {
        let mut value_ptr: *mut ffi::c_void = ptr::null_mut();
        let attr_cfstring = CFString::from_static_string(attribute);

        let result = AXUIElementCopyAttributeValue(
            window,
            attr_cfstring.as_concrete_TypeRef(),
            std::ptr::from_mut::<*mut ffi::c_void>(&mut value_ptr).cast::<*const ffi::c_void>(),
        );

        if result != 0 || value_ptr.is_null() {
            return Err(());
        }

        // If we got a value, just print its debug representation
        if !value_ptr.is_null() {
            let cf_value = CFType::wrap_under_create_rule(value_ptr);
            Ok(Some(format!("{:#?}", cf_value)))
        } else {
            Ok(None)
        }
    }
}
