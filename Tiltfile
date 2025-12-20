# vim: set filetype=starlark :
allow_k8s_contexts(['k3d-stickerbomb'])
default_registry('localhost:5555')

docker_build(
    'stickerbomb',
    '.',
    dockerfile='Dockerfile',
    build_args={'BUILD_PROFILE': 'debug'},
)

k8s_yaml(helm(
    'charts/stickerbomb',
    name='stickerbomb',
    namespace='stickerbomb-system',
    values=['charts/stickerbomb/values-dev.yaml'],
))

k8s_yaml(blob('''
apiVersion: v1
kind: Namespace
metadata:
  name: stickerbomb-system
'''))

k8s_resource(
    new_name='stickerbomb-infrastructure',
    objects=[
        'stickerbomb-system:namespace',
        'labelers.stickerbomb.dev:customresourcedefinition',
    ],
    labels=['setup'],
)

local_resource(
    'generate-crds',
    cmd='mise run generate-crds',
    deps=['crates/crd/src'],
    labels=['setup'],
)

local_resource(
    'apply-sample',
    cmd='kubectl apply -f examples/sample-labeler.yaml',
    resource_deps=['stickerbomb'],
    auto_init=False,
    trigger_mode=TRIGGER_MODE_MANUAL,
    labels=['examples'],
)

k8s_resource(
    workload='stickerbomb',
    port_forwards=['8080:8080'],
    labels=['operator'],
    resource_deps=['generate-crds', 'stickerbomb-infrastructure'],
    objects=[
        'stickerbomb:serviceaccount',
        'stickerbomb:clusterrole',
        'stickerbomb:clusterrolebinding',
    ],
)
