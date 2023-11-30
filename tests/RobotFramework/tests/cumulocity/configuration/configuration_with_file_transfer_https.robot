*** Settings ***
Resource            ../../../resources/common.resource
Library             ThinEdgeIO
Library             Cumulocity
Library             OperatingSystem

Suite Setup         Suite Setup
Suite Teardown      Suite Teardown
Test Setup          Test Setup

Test Tags          theme:configuration    theme:childdevices

*** Variables ***
${PARENT_SN}
${CHILD_SN}


*** Test Cases ***
File Transfer Service has HTTPS enabled
    ThinEdgeIO.Set Device Context  ${PARENT_SN}
    ${code}=    Execute Command     curl --output /dev/null --write-out \%\{http_code\} https://localhost:8000/tedge/file-transfer/non-existent-file     timeout=0
    Should Be Equal     ${code}     404

File Transfer Service redirects HTTP to HTTPS
    ThinEdgeIO.Set Device Context  ${PARENT_SN}
    ${code}=    Execute Command     curl --output /dev/null --write-out \%\{http_code\} http://localhost:8000/tedge/file-transfer/non-existent-file     timeout=0
    Should Be Equal     ${code}     307
    ${GET_url_effective}=    Execute Command     curl --output /dev/null --write-out \%\{url_effective\} -L http://localhost:8000/tedge/file-transfer/non-existent-file     timeout=0
    Should Be Equal     ${GET_url_effective}     https://localhost:8000/tedge/file-transfer/non-existent-file
    ${HEAD_url_effective}=    Execute Command     curl --head --output /dev/null --write-out \%\{url_effective\} -L http://localhost:8000/tedge/file-transfer/non-existent-file     timeout=0
    Should Be Equal     ${HEAD_url_effective}     https://localhost:8000/tedge/file-transfer/non-existent-file

File Transfer Service is accessible over HTTPS from child device
    ThinEdgeIO.Set Device Context  ${CHILD_SN}
    ${code}=    Execute Command     curl --output /dev/null --write-out \%\{http_code\} https://${parent_ip}:8000/tedge/file-transfer/non-existent-file     timeout=0
    Should Be Equal     ${code}     404

Configuration snapshots are uploaded to File Transfer Service via HTTPS
    Get Configuration Should Succeed    device=${CHILD_SN}    external_id=${PARENT_SN}:device:${CHILD_SN}

Configuration snapshots are uploaded to File Transfer Service via HTTPS with client certificate
    Enable Certificate Authentication for File Transfer Service
    Get Configuration Should Succeed    device=${CHILD_SN}    external_id=${PARENT_SN}:device:${CHILD_SN}

Configuration operation fails when configuration-plugin does not supply client certificate
    Enable Certificate Authentication for File Transfer Service
    Disable HTTP Client Certificate for Child Device
    Get Configuration Should Fail
    ...  device=${CHILD_SN}
    ...  failure_reason=config-manager failed uploading configuration snapshot:.+https://${parent_ip}:8000/tedge/file-transfer/.+received fatal alert: CertificateRequired
    ...  external_id=${PARENT_SN}:device:${CHILD_SN}
    Update Configuration Should Fail
    ...  device=${CHILD_SN}
    ...  failure_reason=config-manager failed downloading a file:.+https://${parent_ip}:8000/tedge/file-transfer/.+received fatal alert: CertificateRequired
    ...  external_id=${PARENT_SN}:device:${CHILD_SN}

Configuration snapshot fails when mapper does not supply client certificate
    Enable Certificate Authentication for File Transfer Service
    Disable HTTP Client Certificate for Mapper
    Get Configuration Should Fail
    ...  device=${CHILD_SN}
    ...  failure_reason=tedge-mapper-c8y failed to download configuration snapshot from file-transfer service:.+https://${parent_ip}:8000/tedge/file-transfer/.+received fatal alert: CertificateRequired
    ...  external_id=${PARENT_SN}:device:${CHILD_SN}
    [Teardown]  Re-enable HTTP Client Certificate for Mapper

*** Keywords ***
Get Configuration Should Succeed
    [Arguments]    ${device}    ${external_id}
    Cumulocity.Set Device    ${external_id}
    ${operation}=    Cumulocity.Get Configuration    CONFIG1
    ${operation}=    Operation Should Be SUCCESSFUL    ${operation}    timeout=120

    ThinEdgeIO.Set Device Context    ${device}
    ${expected_checksum}=    Execute Command    md5sum '/etc/config1.json' | cut -d' ' -f1    strip=${True}
    ${events}=    Cumulocity.Device Should Have Event/s    minimum=1    type=CONFIG1    with_attachment=${True}
    ${contents}=    Cumulocity.Event Should Have An Attachment    ${events[0]["id"]}    expected_md5=${expected_checksum}
    RETURN    ${contents}

Get Configuration Should Fail
    [Arguments]    ${failure_reason}    ${device}    ${external_id}
    Cumulocity.Set Device    ${external_id}
    ${operation}=    Cumulocity.Get Configuration    CONFIG1
    Operation Should Be FAILED    ${operation}    failure_reason=${failure_reason}   timeout=120

Update Configuration Should Fail
    [Arguments]    ${failure_reason}    ${device}    ${external_id}
    Cumulocity.Set Device    ${external_id}
    Cumulocity.Should Support Configurations    tedge-configuration-plugin    /etc/tedge/tedge.toml    system.toml    CONFIG1    CONFIG1_BINARY
    ${config_url}=    Cumulocity.Create Inventory Binary    tedge-configuration-plugin    tedge-configuration-plugin    file=${CURDIR}/tedge-configuration-plugin-updated.toml
    ${operation}=    Cumulocity.Set Configuration    CONFIG1    ${config_url}
    Operation Should Be FAILED    ${operation}    failure_reason=${failure_reason}   timeout=120
    Cumulocity.Should Support Configurations    tedge-configuration-plugin    /etc/tedge/tedge.toml    system.toml    CONFIG1    CONFIG1_BINARY

Enable Certificate Authentication for File Transfer Service
    Set Device Context  ${PARENT_SN}
    Execute Command    sudo tedge config set http.ca_path /etc/tedge/device-local-certs/roots
    Execute Command    sudo systemctl restart tedge-agent
    ThinEdgeIO.Service Health Status Should Be Up    tedge-agent

Disable HTTP Client Certificate for Child Device
    Set Device Context  ${CHILD_SN}
    Execute Command    tedge config unset http.client.auth.cert_file
    Execute Command    tedge config unset http.client.auth.key_file
    Execute Command    sudo systemctl restart tedge-configuration-plugin
    ThinEdgeIO.Service Health Status Should Be Up    tedge-configuration-plugin     device=${CHILD_SN}

Disable HTTP Client Certificate for Mapper
    Set Device Context  ${PARENT_SN}
    Execute Command    tedge config unset http.client.auth.cert_file
    Execute Command    tedge config unset http.client.auth.key_file
    ThinEdgeIO.Service Health Status Should Be Up    tedge-mapper-c8y
    Execute Command    sudo systemctl restart tedge-mapper-c8y
    ThinEdgeIO.Service Health Status Should Be Up    tedge-mapper-c8y

Re-enable HTTP Client Certificate for Mapper
    Set Device Context  ${PARENT_SN}
    Execute Command    tedge config set http.client.auth.cert_file /etc/tedge/device-local-certs/main-agent.crt
    Execute Command    tedge config set http.client.auth.key_file /etc/tedge/device-local-certs/main-agent.key
    ThinEdgeIO.Service Health Status Should Be Up    tedge-mapper-c8y
    Execute Command    sudo systemctl restart tedge-mapper-c8y
    ThinEdgeIO.Service Health Status Should Be Up    tedge-mapper-c8y

##
## Setup
##
Suite Setup
    # Parent
    ${parent_sn}=    Setup    skip_bootstrap=${False}
    Set Suite Variable    $PARENT_SN    ${parent_sn}

    ${parent_ip}=    Get IP Address
    Set Suite Variable    $PARENT_IP    ${parent_ip}
    Execute Command    sudo tedge config set mqtt.external.bind.address ${parent_ip}
    Execute Command    sudo tedge config set mqtt.external.bind.port 1883
    Execute Command    sudo tedge config set http.bind.address 0.0.0.0
    Execute Command    sudo tedge config set http.client.host ${parent_ip}

    ThinEdgeIO.Transfer To Device    ${CURDIR}/generate_certificates.sh    /etc/tedge/
    Execute Command    /etc/tedge/generate_certificates.sh  timeout=0
    ${root_certificate}=    Execute Command    cat /etc/tedge/device-local-certs/roots/tedge-local-ca.crt
    ${client_certificate}=    Execute Command    cat /etc/tedge/device-local-certs/tedge-client.crt
    ${client_key}=    Execute Command    cat /etc/tedge/device-local-certs/tedge-client.key

    ThinEdgeIO.Disconnect Then Connect Mapper    c8y
    ThinEdgeIO.Service Health Status Should Be Up    tedge-mapper-c8y

    # Child
    Setup Child Device    parent_ip=${parent_ip}   install_package=tedge-configuration-plugin  root_certificate=${root_certificate}   certificate=${client_certificate}    private_key=${client_key}

Suite Teardown
    Get Logs    name=${PARENT_SN}
    Get Logs    name=${CHILD_SN}

Setup Child Device
    [Arguments]    ${parent_ip}   ${install_package}   ${root_certificate}    ${certificate}   ${private_key}
    ${child_sn}=    Setup    skip_bootstrap=${True}
    Set Suite Variable    $CHILD_SN    ${child_sn}

    Set Device Context    ${CHILD_SN}
    Execute Command    sudo dpkg -i packages/tedge_*.deb

    Execute Command    sudo tedge config set mqtt.client.host ${parent_ip}
    Execute Command    sudo tedge config set mqtt.client.port 1883
    Execute Command    sudo tedge config set http.client.host ${parent_ip}
    Execute Command    sudo tedge config set mqtt.topic_root te
    Execute Command    sudo tedge config set mqtt.device_topic_id device/${child_sn}//

    Execute Command    mkdir -p /etc/tedge/device-local-certs
    Execute Command    echo "${root_certificate}" > /usr/local/share/ca-certificates/tedge-ca.crt
    Execute Command    sudo update-ca-certificates
    Execute Command    tedge config set http.client.auth.cert_file /etc/tedge/device-local-certs/tedge-client.crt
    Execute Command    tedge config set http.client.auth.key_file /etc/tedge/device-local-certs/tedge-client.key
    Execute Command    echo "${certificate}" | sudo tee "$(tedge config get http.client.auth.cert_file)"
    Execute Command    echo "${private_key}" | sudo tee "$(tedge config get http.client.auth.key_file)"
    Execute Command    chown -R tedge:tedge /etc/tedge/device-local-certs

    # Install plugin after the default settings have been updated to prevent it from starting up as the main plugin
    Execute Command    sudo dpkg -i packages/${install_package}*.deb
    Execute Command    sudo systemctl enable ${install_package}
    Execute Command    sudo systemctl start ${install_package}

    Copy Configuration Files    ${child_sn}

    RETURN    ${child_sn}

Test Setup
    Copy Configuration Files    ${PARENT_SN}
    Copy Configuration Files    ${CHILD_SN}
    ThinEdgeIO.Set Device Context    ${CHILD_SN}
    Execute Command    tedge config set http.client.auth.cert_file /etc/tedge/device-local-certs/tedge-client.crt
    Execute Command    tedge config set http.client.auth.key_file /etc/tedge/device-local-certs/tedge-client.key
    Execute Command    sudo systemctl restart tedge-configuration-plugin
    ThinEdgeIO.Service Health Status Should Be Up    tedge-configuration-plugin     device=${CHILD_SN}

Copy Configuration Files
    [Arguments]    ${device}
    ThinEdgeIO.Set Device Context    ${device}
    ThinEdgeIO.Transfer To Device    ${CURDIR}/tedge-configuration-plugin.toml    /etc/tedge/plugins/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/config1.json         /etc/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/config2.json         /etc/
    ThinEdgeIO.Transfer To Device    ${CURDIR}/binary-config1.tar.gz         /etc/

Publish and Verify Local Command
    [Arguments]    ${topic}    ${payload}    ${expected_status}=successful    ${c8y_fragment}=
    [Teardown]    Execute Command    tedge mqtt pub --retain '${topic}' ''
    Execute Command    tedge mqtt pub --retain '${topic}' '${payload}'
    ${messages}=    Should Have MQTT Messages    ${topic}    minimum=1    maximum=1    message_contains="status":"${expected_status}"

    Sleep    5s    reason=Given mapper a chance to react, if it does not react with 5 seconds it never will
    ${retained_message}    Execute Command    timeout 1 tedge mqtt sub --no-topic '${topic}'    ignore_exit_code=${True}    strip=${True}
    Should Be Equal    ${messages[0]}    ${retained_message}    msg=MQTT message should be unchanged

    IF    "${c8y_fragment}"
        # There should not be any c8y related operation transition messages sent: https://cumulocity.com/guides/reference/smartrest-two/#updating-operations
        Should Have MQTT Messages    c8y/s/ds    message_pattern=^(501|502|503),${c8y_fragment}.*    minimum=0    maximum=0
    END
