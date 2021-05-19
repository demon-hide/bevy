use cgmath::EuclideanSpace;
use std::{f64::consts::PI, path::PathBuf};
use structopt::StructOpt;
// ahmed additionals
use bevy_terra::Terrain;


#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long, default_value = "8FH495PF+29")]
    plus: String,
    #[structopt(short, long, default_value = "0")]
    heading: f64,
    #[structopt(short, long, default_value = "200000")]
    elevation: f64,
    #[structopt(long)]
    generate: Option<PathBuf>,
}

fn compute_projection_matrix(width: f32, height: f32) -> cgmath::Matrix4<f32> {
    let aspect = width / height;
    let f = 1.0 / (45.0f32.to_radians() / aspect).tan();
    let near = 0.1;

    #[cfg_attr(rustfmt, rustfmt_skip)]
        cgmath::Matrix4::new(
        f/aspect,  0.0,  0.0,   0.0,
        0.0,       f,    0.0,   0.0,
        0.0,       0.0,  0.0,  -1.0,
        0.0,       0.0,  near,  0.0)
}

fn terra_main(device: &wgpu::Device,
              queue: &wgpu::Queue,
              frame: &wgpu::TextureView,
              depth_buffer: &wgpu::TextureView,
             size: (u32,u32)) -> Terrain {

    let runtime = tokio::runtime::Runtime::new().unwrap();


    let opt = Opt::from_args();



    let mut terrain = bevy_terra::Terrain::new(&device, &queue).unwrap();

    if let Some(dataset_directory) = opt.generate {
        let pb = indicatif::ProgressBar::new(100);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg} {pos}/{len} [{wide_bar}] {percent}% {per_sec} {eta}")
                .progress_chars("=> "),
        );
        let mut last_message = None;
        let mut progress_callback = |l: &str, i: usize, total: usize| {
            if last_message.is_none() || l != last_message.as_ref().unwrap() {
                pb.set_message(l);
                pb.reset_eta();
                last_message = Some(l.to_string());
            }
            pb.set_length(total as u64);
            pb.set_position(i as u64);
        };

        runtime
            .block_on(terrain.generate_heightmaps(
                dataset_directory.join("ETOPO1_Ice_c_geotiff.zip"),
                dataset_directory.join("strm3"),
                &mut progress_callback,
            ))
            .unwrap();
        runtime
            .block_on(
                terrain
                    .generate_albedos(dataset_directory.join("bluemarble"), &mut progress_callback),
            )
            .unwrap();
        runtime.block_on(terrain.generate_roughness(&mut progress_callback)).unwrap();
    }

    terrain


}

fn terra_render(device: &wgpu::Device,
              queue: &wgpu::Queue,
              frame: &wgpu::TextureView,
              depth_buffer: &wgpu::TextureView,
              size: (u32,u32),
              terrain: &mut Terrain) {





    let plus_center = cgmath::Point3::new(0.0f64, 0.0f64, 0.0f64);


    let planet_radius = 6371000.0;
    let mut angle = 0f64.to_radians();
    let mut lat = plus_center.y.to_radians();
    let mut long = plus_center.x.to_radians();
    let mut altitude = 100.0f64;//opt.elevation;

    lat = lat.max(-PI).min(PI);
    if long < -PI {
        long += PI * 2.0;
    }
    if long > PI {
        long -= PI * 2.0;
    }

    let surface_height = terrain.get_height(lat, long) as f64;
    let r = altitude + planet_radius + surface_height + 2.0;
    let eye = cgmath::Point3::new(
        r * lat.cos() * long.cos(),
        r * lat.cos() * long.sin(),
        r * lat.sin(),
    );

    let dt = (planet_radius / (planet_radius + altitude)).acos() * 0.3;
    let latc = lat + angle.cos() * dt;
    let longc = long - angle.sin() * dt;

    let center = cgmath::Point3::new(
        planet_radius * latc.cos() * longc.cos() - eye.x,
        planet_radius * latc.cos() * longc.sin() - eye.y,
        planet_radius * latc.sin() - eye.z,
    );
    let up = cgmath::Vector3::new(eye.x as f32, eye.y as f32, eye.z as f32);

    let view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::origin(),
        cgmath::Point3::new(center.x as f32, center.y as f32, center.z as f32),
        up,
    );

    let proj = compute_projection_matrix(size.0 as f32, size.1 as f32);
    let view_proj = proj * view;
    let view_proj = mint::ColumnMatrix4 {
        x: view_proj.x.into(),
        y: view_proj.y.into(),
        z: view_proj.z.into(),
        w: view_proj.w.into(),
    };


    terrain.render(
        &device,
        &queue,
        &*frame,
        depth_buffer,
        (size.0, size.1),
        view_proj,
        eye.into(),
    );
}