*** Settings ***
Resource            ../../../resources/common.resource
Library             ThinEdgeIO
Library             Cumulocity
Library             OperatingSystem

Force Tags          theme:tedge_agent
Suite Setup         Custom Setup
Test Teardown       Get Logs

*** Test Cases ***

Trigger Custom Download Operation
    Execute Command    tedge mqtt pub --retain te/device/main///cmd/download/robot-123 '{"status":"init","url":"https://from/there","file":"/put/it/here"}'
    ${cmd_messages}    Should Have MQTT Messages    te/device/main///cmd/download/robot-123    message_pattern=.*successful.*   maximum=1
    Execute Command    tedge mqtt pub --retain te/device/main///cmd/download/robot-123 ''
    ${actual_log}      Execute Command    cat /tmp/download-robot-123
    ${expected_log}    Get File    ${CURDIR}/download-command-expected.log
    Should Be Equal    ${actual_log}    ${expected_log}

Override Built-In Operation
    Execute Command     tedge mqtt pub --retain te/device/main///cmd/software_list/robot-456 '{"status":"init"}'
    ${software_list}    Should Have MQTT Messages    te/device/main///cmd/software_list/robot-456    message_pattern=.*successful.*   maximum=1
    Should Contain      ${software_list[0]}    "currentSoftwareList"
    Should Contain      ${software_list[0]}    "mosquitto"
    Should Contain      ${software_list[0]}    "tedge"
    Execute Command     tedge mqtt pub --retain te/device/main///cmd/software_list/robot-456 ''

Trigger A Restart
    Execute Command     tedge mqtt pub --retain te/device/main///cmd/controlled_restart/robot-789 '{"status":"init"}'
    ${cmd_outcome}      Should Have MQTT Messages    te/device/main///cmd/controlled_restart/robot-789    message_pattern=.*successful.*   maximum=2
    ${actual_log}       Execute Command    cat /etc/tedge/operations/restart-robot-789
    ${expected_log}     Get File    ${CURDIR}/restart-command-expected.log
    Should Be Equal     ${actual_log}    ${expected_log}

*** Keywords ***

Custom Setup
    ${DEVICE_SN}=    Setup
    Set Suite Variable    $DEVICE_SN
    Device Should Exist                      ${DEVICE_SN}
    Copy Configuration Files
    Restart Service    tedge-agent

Copy Configuration Files
    ThinEdgeIO.Transfer To Device    ${CURDIR}/software_list.toml       /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/init-software-list.sh    /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/custom-download.toml     /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/schedule-download.sh     /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/launch-download.sh       /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/check-download.sh        /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/custom_restart.toml      /etc/tedge/operations/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/log-restart.sh           /etc/tedge/operations/
