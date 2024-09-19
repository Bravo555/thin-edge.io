//! Definitions of Smartrest 2.0 static templates.
//!
//! https://cumulocity.com/docs/smartrest/mqtt-static-templates/

use std::ops::Deref;

pub struct TemplateId(&'static str);

impl Deref for TemplateId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

// #### Templates quick reference {#templates-quick-reference}
// ### Automatic device creation {#automatic-device-creation}
// ### Handling non-mandatory parameters {#handling-non-mandatory-parameters}

// ### Publish templates {#publish-templates}
// #### Inventory templates (1xx) {#inventory-templates}

// Inventory templates

pub const DEVICE_CREATION: TemplateId = TemplateId("100");
pub const CHILD_DEVICE_CREATION: TemplateId = TemplateId("101");
pub const SERVICE_CREATION: TemplateId = TemplateId("102");
pub const SERVICE_STATUS_UPDATE: TemplateId = TemplateId("104");
pub const GET_CHILD_DEVICES: TemplateId = TemplateId("105");
pub const CLEAR_DEVICE_FRAGMENT: TemplateId = TemplateId("107");
pub const CONFIGURE_HARDWARE: TemplateId = TemplateId("110");
pub const CONFIGURE_MOBILE: TemplateId = TemplateId("111");
pub const CONFIGURE_POSITION: TemplateId = TemplateId("112");
pub const SET_CONFIGURATION: TemplateId = TemplateId("113");
pub const SET_SUPPORTED_OPERATIONS: TemplateId = TemplateId("114");
pub const SET_FIRMWARE: TemplateId = TemplateId("115");
pub const SET_SOFTWARE_LIST: TemplateId = TemplateId("116");
pub const SET_REQUIRED_AVAILABILITY: TemplateId = TemplateId("117");
pub const SET_SUPPORTED_LOGS: TemplateId = TemplateId("118");
pub const SET_SUPPORTED_CONFIGURATIONS: TemplateId = TemplateId("119");
pub const SET_CURRENTLY_INSTALLED_CONFIGURATION: TemplateId = TemplateId("120");
pub const SET_DEVICE_PROFILE_THAT_IS_BEING_APPLIED: TemplateId = TemplateId("121");
pub const SET_DEVICE_AGENT_INFORMATION: TemplateId = TemplateId("122");
pub const SEND_HEARTBEAT: TemplateId = TemplateId("125");
pub const SET_ADVANCED_SOFTWARE_LIST: TemplateId = TemplateId("140");
pub const GET_THE_DEVICE_MANAGED_OBJECT_ID: TemplateId = TemplateId("123");
pub const APPEND_ADVANCED_SOFTWARE_ITEMS: TemplateId = TemplateId("141");
pub const REMOVE_ADVANCED_SOFTWARE_ITEMS: TemplateId = TemplateId("142");
pub const SET_SUPPORTED_SOFTWARE_TYPES: TemplateId = TemplateId("143");
pub const SET_CLOUD_REMOTE_ACCESS: TemplateId = TemplateId("150");

// #### Measurement templates (2xx) {#measurement-templates}

// ##### Create custom measurement (200) {#200}
// ##### Create a custom measurement with multiple fragments and series (201) {#201}
// ##### Create signal strength measurement (210) {#210}
// ##### Create temperature measurement (211) {#211}
// ##### Create battery measurement (212) {#212}

// #### Alarm templates (3xx) {#alarm-templates}

// ##### Create CRITICAL alarm (301) {#301}
// ##### Create MAJOR alarm (302) {#302}
// ##### Create MINOR alarm (303) {#303}
// ##### Create WARNING alarm (304) {#304}
// ##### Update severity of existing alarm (305) {#305}
// ##### Clear existing alarm (306) {#306}
// ##### Clear alarm's fragment (307) {#307}

// #### Event templates (4xx) {#event-templates}

// ##### Create basic event (400) {#400}
// ##### Create location update event (401) {#401}
// ##### Create location update event with device update (402) {#402}
// ##### Clear event's fragment (407) {#407}

// #### Operation templates (5xx) {#operation-templates}

// ##### Get PENDING operations (500) {#500}
// ##### Set operation to EXECUTING (501) {#501}
// ##### Set operation to FAILED (502) {#502}
// ##### Set operation to SUCCESSFUL (503) {#503}
// ##### Set operation to EXECUTING (504) {#504}
// ##### Set operation to FAILED (505) {#505}
// ##### Set operation to SUCCESSFUL (506) {#506}
// ##### Set EXECUTING operations to FAILED (507) {#507}

// ### Subscribe templates {#subscribe-templates}

// #### Inventory templates (1xx) {#inventory-templates-1xx}

// ##### Get children of device (106) {#106}
// ##### Get the device managed object ID (124) {#124}

// #### Operation templates (5xx) {#subscribe-operations}

// ##### Restart (510) {#510}
// ##### Command (511) {#511}
// ##### Configuration (513) {#513}
// ##### Firmware (515) {#515}
// ##### Software list (516) {#516}
// ##### Measurement request operation (517) {#517}
// ##### Relay (518) {#518}
// ##### RelayArray (519) {#519}
// ##### Upload configuration file (520) {#520}
// ##### Download configuration file (521) {#521}
// ##### Logfile request (522) {#522}
// ##### Communication mode (523) {#523}
// ##### Download configuration file with type (524) {#524}
// ##### Firmware from patch (525) {#525}
// ##### Upload configuration file with type (526) {#526}
// ##### Set device profiles (527) {#527}
// ##### Update software (528) {#528}
// ##### Update advanced software (529) {#529}
// ##### Cloud Remote Access connect (530) {#530}
