//! Whether a pipeline loaded from a pipeline library in another process
//! reads through the static samplers it was made with, depending on which
//! static samplers the process made before it.
//!
//! `sampler_order_probe <store|load|make> <library file> [others N] [preload]`
//!
//! The probe's pipeline samples a 2x1 texture (0, 1) through three static
//! samplers: point clamp at u = 0.6 (1.0), linear clamp at u = 0.6 (0.7),
//! linear wrap at u = 0.9 (0.7). `others N` makes N root signatures with
//! other static samplers first; `preload` makes one with the probe's three
//! samplers before anything else.
#![allow(clippy::undocumented_unsafe_blocks)]

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::core::{Interface, HSTRING, PCWSTR, s};
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_FEATURE_LEVEL_12_0};
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("make");
    let file = args.get(2).cloned().unwrap_or_else(|| "probe-library.bin".into());
    let others: usize = args.iter().position(|a| a == "others").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(0);
    let preload = args.iter().any(|a| a == "preload");
    let direct = args.iter().any(|a| a == "direct");
    let dxil: Option<Vec<u8>> = args.iter().position(|a| a == "dxil").and_then(|i| std::fs::read(args.get(i + 1)?).ok());

    let sampler = |filter: D3D12_FILTER, address: D3D12_TEXTURE_ADDRESS_MODE, bias: f32, register: u32| D3D12_STATIC_SAMPLER_DESC {
        Filter: filter,
        AddressU: address,
        AddressV: address,
        AddressW: address,
        MipLODBias: bias,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
        ShaderRegister: register,
        RegisterSpace: 0,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    };
    let probe_samplers = [
        sampler(D3D12_FILTER_MIN_MAG_MIP_POINT, D3D12_TEXTURE_ADDRESS_MODE_CLAMP, 0.0, 0),
        sampler(D3D12_FILTER_MIN_MAG_MIP_LINEAR, D3D12_TEXTURE_ADDRESS_MODE_CLAMP, 0.0, 1),
        sampler(D3D12_FILTER_MIN_MAG_MIP_LINEAR, D3D12_TEXTURE_ADDRESS_MODE_WRAP, 0.0, 2),
    ];

    unsafe {
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_12_0, &mut device)?;
        let device = device.unwrap();

        let root = |params: &[D3D12_ROOT_PARAMETER1], samplers: &[D3D12_STATIC_SAMPLER_DESC]| -> windows::core::Result<ID3D12RootSignature> {
            let desc = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
                Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
                Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                    Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                        NumParameters: params.len() as u32,
                        pParameters: params.as_ptr(),
                        NumStaticSamplers: samplers.len() as u32,
                        pStaticSamplers: samplers.as_ptr(),
                        Flags: if direct { D3D12_ROOT_SIGNATURE_FLAG_CBV_SRV_UAV_HEAP_DIRECTLY_INDEXED | D3D12_ROOT_SIGNATURE_FLAG_SAMPLER_HEAP_DIRECTLY_INDEXED } else { D3D12_ROOT_SIGNATURE_FLAG_NONE },
                    },
                },
            };
            let mut blob: Option<ID3DBlob> = None;
            D3D12SerializeVersionedRootSignature(&desc, &mut blob, None)?;
            let blob = blob.unwrap();
            let bytes = std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize());
            device.CreateRootSignature(0, bytes)
        };

        let mut kept = Vec::new();
        if preload {
            kept.push(root(&[], &probe_samplers)?);
        }
        let other_pso: Option<Vec<u8>> = args.iter().position(|a| a == "otherpso").and_then(|i| std::fs::read(args.get(i + 1)?).ok());
        let mut kept_psos: Vec<ID3D12PipelineState> = Vec::new();

        let range = D3D12_DESCRIPTOR_RANGE1 {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            Flags: D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC,
            OffsetInDescriptorsFromTableStart: 0,
        };
        let params = [
            D3D12_ROOT_PARAMETER1 {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                Anonymous: D3D12_ROOT_PARAMETER1_0 {
                    DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 { NumDescriptorRanges: 1, pDescriptorRanges: &range },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
            },
            D3D12_ROOT_PARAMETER1 {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_UAV,
                Anonymous: D3D12_ROOT_PARAMETER1_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR1 { ShaderRegister: 0, RegisterSpace: 0, Flags: D3D12_ROOT_DESCRIPTOR_FLAG_NONE },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
            },
        ];
        for i in 0..others {
            let filter = [D3D12_FILTER_MIN_MAG_MIP_POINT, D3D12_FILTER_MIN_MAG_MIP_LINEAR, D3D12_FILTER_ANISOTROPIC][i % 3];
            let address = [D3D12_TEXTURE_ADDRESS_MODE_MIRROR, D3D12_TEXTURE_ADDRESS_MODE_BORDER][i % 2];
            let r = root(&params, &[sampler(filter, address, 0.25 * (i + 1) as f32, 0)])?;
            if let Some(code) = &other_pso {
                kept_psos.push(device.CreateComputePipelineState(&D3D12_COMPUTE_PIPELINE_STATE_DESC {
                    pRootSignature: std::mem::transmute_copy(&r),
                    CS: D3D12_SHADER_BYTECODE { pShaderBytecode: code.as_ptr().cast(), BytecodeLength: code.len() },
                    ..Default::default()
                })?);
            }
            kept.push(r);
        }
        let probe_root = root(&params, &probe_samplers)?;

        let source = "Texture2D<float> t : register(t0);\n\
            SamplerState sp : register(s0); SamplerState sl : register(s1); SamplerState sw : register(s2);\n\
            RWByteAddressBuffer o : register(u0);\n\
            [numthreads(1, 1, 1)] void main() {\n\
              o.Store(0, asuint(t.SampleLevel(sp, float2(0.6, 0.5), 0)));\n\
              o.Store(4, asuint(t.SampleLevel(sl, float2(0.6, 0.5), 0)));\n\
              o.Store(8, asuint(t.SampleLevel(sw, float2(0.9, 0.5), 0)));\n\
            }";
        let mut code: Option<ID3DBlob> = None;
        D3DCompile(source.as_ptr().cast(), source.len(), s!("probe"), None, None, s!("main"), s!("cs_5_1"), 0, 0, &mut code, None)?;
        let code = code.unwrap();
        let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&probe_root),
            CS: match &dxil {
                Some(d) => D3D12_SHADER_BYTECODE { pShaderBytecode: d.as_ptr().cast(), BytecodeLength: d.len() },
                None => D3D12_SHADER_BYTECODE { pShaderBytecode: code.GetBufferPointer(), BytecodeLength: code.GetBufferSize() },
            },
            ..Default::default()
        };
        let device1: ID3D12Device1 = device.cast()?;
        let name = HSTRING::from("probe");
        let bytes = if mode == "load" { std::fs::read(&file).expect("the library file") } else { Vec::new() };
        let library: ID3D12PipelineLibrary1 = device1.CreatePipelineLibrary::<ID3D12PipelineLibrary>(&bytes)?.cast()?;
        let pso: ID3D12PipelineState = if mode == "load" {
            library.LoadComputePipeline(PCWSTR(name.as_ptr()), &desc)?
        } else {
            device.CreateComputePipelineState(&desc)?
        };
        if mode == "store" {
            library.StorePipeline(PCWSTR(name.as_ptr()), &pso)?;
            let mut out = vec![0u8; library.GetSerializedSize()];
            library.Serialize(&mut out)?;
            std::fs::write(&file, out).expect("the library file written");
        }

        // The texture (0, 1), its view, the output and its read-back.
        let heap = |kind: D3D12_HEAP_TYPE| D3D12_HEAP_PROPERTIES { Type: kind, ..Default::default() };
        let buffer = |size: u64, flags: D3D12_RESOURCE_FLAGS| D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: flags,
            ..Default::default()
        };
        let mut texture: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(
            &heap(D3D12_HEAP_TYPE_DEFAULT),
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: 2,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R32_FLOAT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_COMMON,
            None,
            &mut texture,
        )?;
        let texture = texture.unwrap();
        let mut upload: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&heap(D3D12_HEAP_TYPE_UPLOAD), D3D12_HEAP_FLAG_NONE, &buffer(256, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut upload)?;
        let upload = upload.unwrap();
        let mut p = std::ptr::null_mut();
        upload.Map(0, None, Some(&mut p))?;
        std::ptr::copy_nonoverlapping([0.0f32, 1.0].as_ptr(), p.cast(), 2);
        upload.Unmap(0, None);
        let mut output: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&heap(D3D12_HEAP_TYPE_DEFAULT), D3D12_HEAP_FLAG_NONE, &buffer(256, D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS), D3D12_RESOURCE_STATE_COMMON, None, &mut output)?;
        let output = output.unwrap();
        let mut back: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&heap(D3D12_HEAP_TYPE_READBACK), D3D12_HEAP_FLAG_NONE, &buffer(256, D3D12_RESOURCE_FLAG_NONE), D3D12_RESOURCE_STATE_COPY_DEST, None, &mut back)?;
        let back = back.unwrap();
        let views: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        })?;
        device.CreateShaderResourceView(
            &texture,
            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R32_FLOAT,
                ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MipLevels: 1, ..Default::default() } },
            }),
            views.GetCPUDescriptorHandleForHeapStart(),
        );

        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() })?;
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        list.CopyTextureRegion(
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&texture),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
            },
            0,
            0,
            0,
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&upload),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT { Format: DXGI_FORMAT_R32_FLOAT, Width: 2, Height: 1, Depth: 1, RowPitch: 256 },
                    },
                },
            },
            None,
        );
        let barrier = |resource: &ID3D12Resource, before, after| D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: std::mem::transmute_copy(resource),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        };
        list.ResourceBarrier(&[
            barrier(&texture, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE),
            barrier(&output, D3D12_RESOURCE_STATE_COMMON, D3D12_RESOURCE_STATE_UNORDERED_ACCESS),
        ]);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetComputeRootSignature(&probe_root);
        list.SetPipelineState(&pso);
        list.SetComputeRootDescriptorTable(0, views.GetGPUDescriptorHandleForHeapStart());
        list.SetComputeRootUnorderedAccessView(1, output.GetGPUVirtualAddress());
        list.Dispatch(1, 1, 1);
        list.ResourceBarrier(&[barrier(&output, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        list.CopyResource(&back, &output);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let event = CreateEventW(None, false, false, None)?;
        queue.Signal(&fence, 1)?;
        fence.SetEventOnCompletion(1, event)?;
        WaitForSingleObject(event, INFINITE);
        let mut p = std::ptr::null_mut();
        back.Map(0, None, Some(&mut p))?;
        let got = std::slice::from_raw_parts(p.cast::<f32>(), 3).to_vec();
        back.Unmap(0, None);
        let ok = (got[0] - 1.0).abs() < 1e-3 && (got[1] - 0.7).abs() < 1e-3 && (got[2] - 0.7).abs() < 1e-3;
        println!(
            "{mode} others={others} preload={preload}: point {:.3} (1.000), linear {:.3} (0.700), wrap {:.3} (0.700) -> {}",
            got[0],
            got[1],
            got[2],
            if ok { "RIGHT" } else { "WRONG" }
        );
        drop(kept);
    }
    Ok(())
}
